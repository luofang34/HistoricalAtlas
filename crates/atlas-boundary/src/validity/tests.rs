use super::*;
use atlas_core::identity::EntityId;
use serde_json::json;

#[cfg(test)]
fn dataset() -> (Dataset, FactId, FactId, FactId) {
    let commune = EntityId::from_key("earth/test/commune").unwrap();
    let other = EntityId::from_key("earth/test/other").unwrap();
    let drawing = FactId::from_key("sheet/commune").unwrap();
    let chronology = FactId::from_key("history/commune").unwrap();
    let elsewhere = FactId::from_key("history/other").unwrap();
    let record = |id: FactId, key: &str, entity: EntityId| {
        json!({"id": id, "key": key, "entity_id": entity, "source_id": "s",
            "locator": "test", "checked": true, "observation": {}})
    };
    let data = serde_json::from_value(json!({
        "entities": [
            {"id": commune, "key": "earth/test/commune", "body": "earth", "kind": {"polity": "administrative"}},
            {"id": other, "key": "earth/test/other", "body": "earth", "kind": {"polity": "administrative"}}],
        "records": [record(drawing, "sheet/commune", commune),
            record(chronology, "history/commune", commune),
            record(elsewhere, "history/other", other)],
        "estimates": [], "disputes": []}))
    .unwrap();
    (data, drawing, chronology, elsewhere)
}

#[test]
fn intervals_need_evidence_about_the_same_entity() {
    let (data, drawing, chronology, elsewhere) = dataset();
    let span: TemporalSpan = serde_json::from_value(json!({
        "start": {"earliest": "10", "nominal": "10", "latest": "10", "precision": "year"},
        "end": null}))
    .unwrap();
    assert!(validate(&data, drawing, None, &[]).is_ok());
    assert!(validate(&data, drawing, Some(&span), &[chronology]).is_ok());
    assert!(matches!(
        validate(&data, drawing, Some(&span), &[]),
        Err(ValidityError::Uncited(_))
    ));
    assert!(matches!(
        validate(&data, drawing, Some(&span), &[elsewhere]),
        Err(ValidityError::Basis { .. })
    ));
    assert!(matches!(
        validate(&data, drawing, None, &[chronology]),
        Err(ValidityError::BasisWithoutInterval(_))
    ));
}
