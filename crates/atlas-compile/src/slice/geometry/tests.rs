use super::*;
use atlas_boundary::geographic::GeographicBoundary;
use atlas_core::identity::FactId;
use serde_json::json;
use std::path::PathBuf;

#[test]
fn slice_geometry_preserves_uncertainty_and_excludes_undated_or_absent_entities() {
    let id = EntityId::from_key("moon/test/enclosure").unwrap();
    let record = FactId::from_key("survey/enclosure").unwrap();
    let data: Dataset = serde_json::from_value(json!({
        "estimates":[],"disputes":[],
        "entities":[{"id":id,"key":"moon/test/enclosure","kind":{"infrastructure":"facility"},"body":"moon"}],
        "records":[{"id":record,"key":"survey/enclosure","entity_id":id,"source_id":"survey",
            "page_or_sheet":"A","locator":"Enclosure","checked":true,"observation":{"name":"Enclosure"}}]
    })).unwrap();
    let boundary: GeographicBoundary = serde_json::from_value(json!({
        "unit":{"record_id":record,"polygons":[{"exterior":[0,1,2,3,0],"interiors":[]}],
            "validity":{"start":{"earliest":"10","nominal":"12","latest":"14","precision":"day"},
            "end":{"earliest":"20","nominal":"20","latest":"20","precision":"day"}}},
        "vertices":[{"x":1,"y":1},{"x":2,"y":1},{"x":2,"y":2},{"x":1,"y":2}],
        "datum":"moon_iau","boundary_method":"unstated","method_basis":[],"registration":"maps/survey.json"
    })).unwrap();
    for (ticks, mode, absent, expected) in [
        (9, TimeSelection::Possible, false, None),
        (10, TimeSelection::Possible, false, Some("possible")),
        (12, TimeSelection::Certain, false, None),
        (14, TimeSelection::Certain, false, Some("certain")),
        (14, TimeSelection::Possible, true, None),
        (20, TimeSelection::Possible, false, None),
    ] {
        let mut undated = boundary.clone();
        undated.unit.validity = None;
        let index = json!({id.0.to_string(): {"boundaries":[boundary,undated]}});
        let mut artifacts = Artifacts::from([(
            PathBuf::from("entities/index/00.json"),
            serde_json::to_vec(&index).unwrap(),
        )]);
        let absent = if absent {
            BTreeSet::from([id])
        } else {
            BTreeSet::new()
        };
        let paths = publish(
            &mut artifacts,
            &data,
            EpochTime::from_ticks(ticks),
            mode,
            &absent,
        )
        .unwrap();
        let features: Vec<Value> = paths.iter().flat_map(|path| {
            serde_json::from_slice::<Value>(&artifacts[&PathBuf::from(path)]).unwrap()["features"]
                .as_array().unwrap().clone()
        }).collect();
        assert_eq!(features.len(), usize::from(expected.is_some()));
        if let Some(membership) = expected {
            assert_eq!(features[0]["properties"]["membership"], membership);
            assert_eq!(features[0]["properties"]["datum"], "moon_iau");
        }
    }
}
