use super::*;
use atlas_core::identity::EntityId;

#[test]
fn real_commune_recipe_publishes_every_reconstruction_with_its_source_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let request: DerivationRequest =
        read_json_blocking(&root.join("guizhou-1960s/bijie/communes.derive.json")).unwrap();
    let output = compile_derivation(&input, &request).unwrap();
    assert_eq!(request.units.len(), 124);
    assert!(
        output
            .values()
            .all(|b| b.len() < crate::export::MAX_FILE_BYTES)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &output[Path::new("derived/bijie-communes-terrain.json")]
        )
        .unwrap(),
        serde_json::to_value(&request).unwrap()
    );
    for unit in &request.units {
        let record = input
            .data
            .records
            .iter()
            .find(|r| r.id == unit.record_id)
            .unwrap();
        let path = format!("entities/index/{}.json", record.entity_id.shard());
        let shard: serde_json::Value = serde_json::from_slice(&output[Path::new(&path)]).unwrap();
        let boundary: DerivedBoundary = serde_json::from_value(
            shard[record.entity_id.0.to_string()]["derived_boundaries"][0].clone(),
        )
        .unwrap();
        let feature = boundary.feature(&input.data, None).unwrap();
        assert_eq!(feature["properties"]["reconstructed"], true);
        assert_eq!(feature["properties"]["geometry_kind"], "derived_polygon");
        assert!(feature["properties"]["validity"].is_null());
        assert!(boundary.basis.contains(&request.envelope_record_id));
        assert!(boundary.basis.contains(&unit.record_id));
        assert!(boundary.region.cells > 0);
    }
    let id = EntityId::from_key("earth/cn-gz/bijie/hantun-commune").unwrap();
    let unit = request
        .units
        .iter()
        .find(|u| {
            input
                .data
                .records
                .iter()
                .any(|r| r.id == u.record_id && r.entity_id == id)
        })
        .unwrap();
    assert!(
        unit.anchors
            .iter()
            .any(|a| matches!(a, AnchorReference::MapLocation { .. }))
    );
    assert!(!request.excluded.is_empty());
}

#[test]
fn reconstruction_rejects_other_communes_anchors_and_uncited_exclusions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let request: DerivationRequest =
        read_json_blocking(&root.join("guizhou-1960s/bijie/communes.derive.json")).unwrap();
    let mut invalid = request.clone();
    invalid.units[0].anchors = invalid.units[1].anchors.clone();
    assert!(
        reconstruct(&input, &invalid)
            .unwrap_err()
            .to_string()
            .contains("hierarchy path")
    );
    let mut invalid = request.clone();
    invalid.excluded[0].reason.clear();
    assert!(
        reconstruct(&input, &invalid)
            .unwrap_err()
            .to_string()
            .contains("reason")
    );
    let mut invalid = request;
    invalid.terrain_tiles[0].sha256 = "invalid".into();
    assert!(
        reconstruct(&input, &invalid)
            .unwrap_err()
            .to_string()
            .contains("fingerprint")
    );
}

#[test]
fn reconstruction_rejects_a_seed_whose_review_has_been_withdrawn() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    let request: DerivationRequest =
        read_json_blocking(&root.join("guizhou-1960s/bijie/communes.derive.json")).unwrap();
    let seed = request
        .units
        .iter()
        .flat_map(|unit| &unit.anchors)
        .find_map(|anchor| match anchor {
            AnchorReference::Record { record_id } => Some(*record_id),
            AnchorReference::MapLocation { .. } => None,
        })
        .unwrap();
    input
        .data
        .records
        .iter_mut()
        .find(|r| r.id == seed)
        .unwrap()
        .checked = false;
    assert!(
        reconstruct(&input, &request)
            .unwrap_err()
            .to_string()
            .contains("missing checked")
    );
}
