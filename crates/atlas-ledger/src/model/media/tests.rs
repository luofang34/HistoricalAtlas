use super::*;
use crate::{
    model::{Dataset, SourceCatalog},
    timeline::{TimeSelection, Timeline},
    validation::validate,
};
use atlas_core::{identity::FactId, time::EpochTime};
use serde_json::json;

#[cfg(test)]
fn evidence() -> (SourceCatalog, Dataset) {
    let entity = EntityId::from_key("earth/test/runway").unwrap();
    let record = FactId::from_key("archive/frame-17/runway").unwrap();
    let source = serde_json::from_value(json!({"sources":[{
        "id":"archive", "title":"Air reconnaissance", "capture":{
            "medium":"aerial_photograph", "sortie":"B-4", "frame":"17",
            "archive":{"repository":"Test archive", "accession":"AIR/17"}
        }
    }]}))
    .unwrap();
    let data = serde_json::from_value(json!({"entities":[{"id":entity, "key":"earth/test/runway", "body":"earth", "kind":{"infrastructure":"runway"}}],
        "records":[{"id":record,"key":"archive/frame-17/runway","entity_id":entity,"source_id":"archive","locator":"Runway in frame 17","checked":true,
            "observation":{"physical":{"measurements":[{"kind":"length","metres":1573.0,"notation":"1573 m"}]}}}],
        "estimates":[],"disputes":[]})).unwrap();
    (source, data)
}

#[test]
fn archival_image_assertion_needs_neither_table_cells_nor_a_scan() {
    let (source, data) = evidence();
    validate(&source, &data).unwrap();
    assert!(data.records[0].observation.name.is_empty());
    assert!(data.records[0].observation.population.is_none());
    let mut invalid = source.clone();
    invalid.sources[0]
        .capture
        .as_mut()
        .unwrap()
        .archive
        .as_mut()
        .unwrap()
        .accession
        .clear();
    assert!(validate(&invalid, &data).is_err());
    let mut invalid = data.clone();
    invalid.records[0]
        .observation
        .physical
        .as_mut()
        .unwrap()
        .flows_into = Some(data.entities[0].id);
    assert!(validate(&source, &invalid).is_err());
    invalid.records[0]
        .observation
        .physical
        .as_mut()
        .unwrap()
        .flows_into = None;
    invalid.records[0]
        .observation
        .physical
        .as_mut()
        .unwrap()
        .measurements[0]
        .metres = f64::NAN;
    assert!(validate(&source, &invalid).is_err());
}

#[test]
fn dated_dimensions_and_destruction_survive_time_selection() {
    let (source, mut data) = evidence();
    for (key, value) in [
        (
            "length",
            json!({"property":"measurement","value":{"kind":"length","metres":1800.0,"notation":"1800 m"}}),
        ),
        (
            "destroyed",
            json!({"property":"existence","value":"destroyed"}),
        ),
    ] {
        let key = format!("editorial/runway/{key}");
        data.estimates.push(serde_json::from_value(json!({"id":FactId::from_key(&key).unwrap(),"key":key,
            "entity_id":data.entities[0].id,"basis":[data.records[0].id],"brief":"Test interpretation",
            "value":{"kind":"temporal","assertion":{"validity":{
                "start":{"earliest":"10","nominal":"10","latest":"10","precision":"day"},
                "end":{"earliest":"20","nominal":"20","latest":"20","precision":"day"}},"value":value}}})).unwrap());
    }
    let timeline = Timeline::new(&source, &data).unwrap();
    assert!(
        timeline
            .at(EpochTime::from_ticks(9), TimeSelection::Certain)
            .unwrap()
            .entities
            .is_empty()
    );
    let slice = timeline
        .at(EpochTime::from_ticks(10), TimeSelection::Certain)
        .unwrap();
    assert!(!slice.entities[0].is_present());
    assert_eq!(slice.entities[0].properties.len(), 2);
    assert!(
        timeline
            .at(EpochTime::from_ticks(20), TimeSelection::Certain)
            .unwrap()
            .entities
            .is_empty()
    );
}
