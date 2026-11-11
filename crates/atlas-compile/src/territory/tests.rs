use super::*;
use atlas_boundary::nodal::NodalScope;
use atlas_core::identity::EntityId;
use serde_json::Value;
use std::collections::BTreeSet;

#[cfg(test)]
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

#[cfg(test)]
fn request(name: &str) -> NodalQuery {
    NodalQuery {
        entity_id: EntityId::from_key(&format!("earth/cn-gz/bijie/{name}-commune")).unwrap(),
        scope: NodalScope::Source {
            source_id: "bijie-gazetteer".into(),
        },
    }
}

#[test]
fn real_wuliping_network_preserves_every_table_identity_and_cites_every_connection() {
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let query = request("wuliping");
    let table: atlas_ledger::model::Dataset =
        read_json_blocking(&root().join("guizhou-1960s/bijie/page-35.dataset.json")).unwrap();
    let expected: BTreeSet<_> = table
        .entities
        .iter()
        .map(|entity| entity.id)
        .chain([query.entity_id])
        .collect();
    let territory = NodalBuilder::new(&input.catalog, &input.data)
        .unwrap()
        .build(query.clone())
        .unwrap();
    assert_eq!(
        territory
            .nodes
            .iter()
            .map(|node| node.entity_id)
            .collect::<BTreeSet<_>>(),
        expected
    );
    let artifacts = compile_territory(&input, query).unwrap();
    for link in &territory.links {
        let atlas_boundary::nodal::RelationEvidence::Record { record_id } = link.evidence else {
            panic!("source query emitted temporal evidence")
        };
        let record = input
            .data
            .records
            .iter()
            .find(|record| record.id == record_id)
            .unwrap();
        assert!(record.checked);
        let entry = crate::export::lookup_entity(&artifacts, record.entity_id)
            .unwrap()
            .unwrap();
        assert!(
            entry["records"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value["id"] == record_id.0.to_string())
        );
    }
    let network: Value = serde_json::from_slice(&artifacts[Path::new("territory.json")]).unwrap();
    assert_eq!(network["query"]["scope"]["source_id"], "bijie-gazetteer");
    assert!(
        artifacts
            .values()
            .all(|bytes| bytes.len() < crate::export::MAX_FILE_BYTES)
    );
    let geojson: Value =
        serde_json::from_slice(&artifacts[Path::new(network["features"][0].as_str().unwrap())])
            .unwrap();
    let features = geojson["features"].as_array().unwrap();
    assert!(
        features
            .iter()
            .any(|feature| feature["geometry"]["type"] == "MultiLineString")
    );
    assert!(
        features
            .iter()
            .all(|feature| feature["properties"]["geometry_kind"] == "nodal_territory")
    );
}

#[test]
fn unresolved_yinhe_uses_only_its_cited_seat_as_a_network_anchor() {
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let query = request("toubu");
    let territory = NodalBuilder::new(&input.catalog, &input.data)
        .unwrap()
        .build(query)
        .unwrap();
    let record = input
        .data
        .records
        .iter()
        .find(|record| record.key == "bijie-gazetteer/p38/toubu/yinhe-brigade")
        .unwrap();
    let node = territory
        .nodes
        .iter()
        .find(|node| node.entity_id == record.entity_id)
        .unwrap();
    assert!(node.anchors.is_empty());
    assert_eq!(node.unresolved_coordinate_records, vec![record.id]);
    assert!(
        territory
            .links
            .iter()
            .any(|link| link.to == record.entity_id)
    );
    let rendered = territory.geojson();
    let marker = rendered["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|feature| feature["properties"]["entity_id"] == record.entity_id.0.to_string())
        .unwrap();
    assert_eq!(
        marker["properties"]["anchor_entity_id"],
        record.observation.seat_id.unwrap().0.to_string()
    );
    assert_eq!(
        marker["properties"]["seat_evidence"]["record_id"],
        record.id.0.to_string()
    );
    assert_eq!(
        marker["properties"]["anchor"]["coordinate"]["longitude_text"],
        "105°20′"
    );
    assert_ne!(
        marker["properties"]["anchor"]["record_id"],
        record.id.0.to_string()
    );
    assert!(
        territory
            .nodes
            .iter()
            .filter(|node| !node.anchors.is_empty())
            .count()
            > 40
    );
}

#[test]
fn bijie_town_retains_dated_rank_evidence_when_its_parent_is_unknown() {
    use atlas_core::calendar::{Calendar, CivilDate};
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|estimate| estimate.key == "editorial/bijie-town/1958-hierarchy")
        .unwrap();
    let query = NodalQuery {
        entity_id: estimate.entity_id,
        scope: NodalScope::Time {
            at: CivilDate {
                year: 1960,
                month: 1,
                day: 1,
            }
            .to_epoch(Calendar::Gregorian, 0)
            .unwrap(),
            selection: atlas_ledger::timeline::TimeSelection::Certain,
        },
    };
    let territory = NodalBuilder::new(&input.catalog, &input.data)
        .unwrap()
        .build(query)
        .unwrap();
    assert_eq!(territory.nodes.len(), 1);
    assert!(territory.links.is_empty());
    let evidence = serde_json::to_value(&territory.nodes[0].evidence).unwrap();
    assert_eq!(evidence[0]["estimate_id"], estimate.id.0.to_string());
    assert_eq!(evidence[0]["membership"], "certain");
}

#[test]
fn territory_command_writes_reproducible_artifacts_and_refuses_existing_output() {
    let request = root().join("guizhou-1960s/bijie/wuliping.territory.json");
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("network");
    let count = territory_blocking(&root(), &request, &output).unwrap();
    assert!(count > 200);
    let result: Value = read_json_blocking(&output.join("territory.json")).unwrap();
    assert_eq!(result["node_count"], 22);
    assert!(output.join("manifest.json").is_file());
    assert!(output.join("sources/000.json").is_file());
    assert!(
        output
            .join(result["features"][0].as_str().unwrap())
            .is_file()
    );
    assert!(territory_blocking(&root(), &request, &output).is_err());
    let bad = directory.path().join("missing.json");
    assert!(territory_blocking(&root(), &bad, &directory.path().join("rejected")).is_err());
    assert!(!directory.path().join("rejected").exists());
}

#[test]
fn published_alias_queries_resolve_to_the_current_network_without_losing_the_request() {
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let canonical = request("wuliping");
    let alias = EntityId::from_key("earth/cn-gz/bijie/retired-wuliping-key").unwrap();
    input.aliases.0.insert(alias, canonical.entity_id);
    let mut query = canonical.clone();
    query.entity_id = alias;
    let artifacts = compile_territory(&input, query).unwrap();
    let manifest: Value = serde_json::from_slice(&artifacts[Path::new("territory.json")]).unwrap();
    assert_eq!(manifest["query"]["entity_id"], alias.0.to_string());
    assert_eq!(manifest["entity_id"], canonical.entity_id.0.to_string());
    assert_eq!(manifest["node_count"], 22);
    let nodes: Value =
        serde_json::from_slice(&artifacts[Path::new(manifest["nodes"][0].as_str().unwrap())])
            .unwrap();
    assert!(
        nodes
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["entity_id"] == canonical.entity_id.0.to_string())
    );
}
