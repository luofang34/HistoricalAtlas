use super::*;
use atlas_core::identity::FactId;
use atlas_ledger::model::{Dataset, SourceCatalog};
use serde_json::json;

#[cfg(test)]
fn input() -> LedgerInput {
    let mut data = Dataset::default();
    for key in ["earth/test/alpha", "earth/test/beta"] {
        let id = EntityId::from_key(key).unwrap();
        let fact = format!("source/{key}");
        data.entities.push(
            serde_json::from_value(json!({"id":id,"key":key,"body":"earth","kind":"settlement"}))
                .unwrap(),
        );
        data.records.push(serde_json::from_value(json!({"id":FactId::from_key(&fact).unwrap(),"key":fact,"entity_id":id,
            "source_id":"test","locator":"Marker","checked":true,"observation":{"name":key,
            "coordinate":{"point":{"longitude":160.0,"latitude":-9.0},"datum":"wgs84","longitude_text":"160 E","latitude_text":"9 S","resolution_arcminutes":1.0}}})).unwrap());
    }
    let catalog: SourceCatalog = serde_json::from_value(
        json!({"sources":[{"id":"test","title":"Test survey","url":"https://example.org/survey"}]}),
    )
    .unwrap();
    LedgerInput {
        data,
        catalog,
        derivations: BTreeMap::new(),
        traces: Vec::new(),
        place_maps: BTreeMap::new(),
        plate_models: BTreeMap::new(),
        aliases: Default::default(),
        published: Default::default(),
    }
}

#[test]
fn deterministic_export_keeps_coincident_entities_and_omits_unchecked_evidence() {
    let mut input = input();
    let output = compile(&input).unwrap();
    assert_eq!(output, compile(&input).unwrap());
    for record in &input.data.records {
        let shard: serde_json::Value = serde_json::from_slice(
            &output[&PathBuf::from(format!("entities/index/{}.json", record.entity_id.shard()))],
        )
        .unwrap();
        assert_eq!(
            shard[record.entity_id.0.to_string()]["records"][0],
            serde_json::to_value(record).unwrap()
        );
    }
    input.data.records[0].checked = false;
    let id = input.data.records[0].entity_id;
    let output = compile(&input).unwrap();
    if let Some(bytes) = output.get(&PathBuf::from(format!(
        "entities/index/{}.json",
        id.shard()
    ))) {
        let shard: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert!(shard.get(id.0.to_string()).is_none());
    }
    input.published.0.insert(id);
    assert!(compile(&input).is_err());
}

#[test]
fn strict_file_budget_rejects_oversize_output() {
    let mut artifacts = Artifacts::new();
    assert!(matches!(
        insert(&mut artifacts, "large.json", &"x".repeat(MAX_FILE_BYTES)),
        Err(ExportError::Oversize { .. })
    ));
    assert!(artifacts.is_empty());
}

#[test]
fn computed_coordinates_survive_json_publication_without_rounding_again() {
    let mut input = input();
    let record = &mut input.data.records[0];
    record
        .observation
        .coordinate
        .as_mut()
        .unwrap()
        .point
        .longitude = 105.87955975133923;
    let id = record.entity_id;
    let expected = serde_json::to_value(&*record).unwrap();
    let output = compile(&input).unwrap();
    let shard: serde_json::Value = serde_json::from_slice(
        &output[&PathBuf::from(format!("entities/index/{}.json", id.shard()))],
    )
    .unwrap();
    assert_eq!(shard[id.0.to_string()]["records"][0], expected);
}

#[test]
fn hierarchy_cannot_publish_a_parent_without_checked_evidence() {
    let mut input = input();
    input.data.records[0].observation.parent_id = Some(input.data.entities[1].id);
    input.data.records[1].checked = false;
    assert!(matches!(
        compile(&input),
        Err(ExportError::MissingEvidence(_))
    ));
}
