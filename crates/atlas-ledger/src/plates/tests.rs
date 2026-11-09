use super::*;
use serde_json::json;

#[cfg(test)]
fn fixture() -> (BTreeMap<String, PlateModelDocument>, Dataset, FactId) {
    let model =
        RotationModel::from_rot("602 10 90 360 10 0 ! South China\n602 0 90 0 0 0\n").unwrap();
    let document = PlateModelDocument {
        id: "test-model".into(),
        source_id: "rotations".into(),
        source_sha256: "a".repeat(64),
        attribution: "Test".into(),
        license: "CC BY 4.0".into(),
        reference_frame: "Test mantle frame".into(),
        present_epoch: EpochTime::from_ticks(0),
        youngest_ma: 0.0,
        oldest_ma: 10.0,
        sequences: model.sequences().cloned().collect(),
    };
    let entity = EntityId::from_key("earth/test/fossil-site").unwrap();
    let record = FactId::from_key("survey/fossil-site").unwrap();
    let estimate = FactId::from_key("interpretation/fossil-site/plate").unwrap();
    let data = serde_json::from_value(json!({
        "entities": [{"id": entity, "key": "earth/test/fossil-site", "body": "earth",
            "kind": "geological_site"}],
        "records": [{"id": record, "key": "survey/fossil-site", "entity_id": entity,
            "source_id": "survey", "locator": "Table 1", "checked": true,
            "observation": {"coordinate": {"point": {"longitude": 100.0, "latitude": 0.0},
                "longitude_text": "E100", "latitude_text": "N0",
                "resolution_arcminutes": 1.0, "datum": "unspecified"}}}],
        "estimates": [{"id": estimate, "key": "interpretation/fossil-site/plate",
            "entity_id": entity, "basis": [record], "brief": "Test assignment",
            "value": {"kind": "plate", "model_id": "test-model", "plate_id": 602}}],
        "disputes": []}))
    .unwrap();
    (
        BTreeMap::from([("test-model".into(), document)]),
        data,
        record,
    )
}

#[test]
fn covered_epochs_rotate_coordinates_and_uncovered_epochs_leave_them_out() {
    let (models, data, record) = fixture();
    let five_ma = EpochTime::from_ticks(-5 * 1_000_000 * JULIAN_YEAR_MS);
    let positions = paleo_positions(&models, &data, five_ma).unwrap();
    assert_eq!(positions.len(), 1);
    let position = &positions[0];
    assert_eq!(position.record_id, record);
    assert!((position.age_ma - 5.0).abs() < 1e-9);
    assert_eq!(position.reference_frame, "Test mantle frame");
    assert!((position.reconstructed.longitude - 105.0).abs() < 1e-9);
    assert_eq!(position.observed.longitude, 100.0);
    let older = EpochTime::from_ticks(-20 * 1_000_000 * JULIAN_YEAR_MS);
    assert!(paleo_positions(&models, &data, older).unwrap().is_empty());
    let future = EpochTime::from_ticks(JULIAN_YEAR_MS);
    assert!(paleo_positions(&models, &data, future).unwrap().is_empty());
}
