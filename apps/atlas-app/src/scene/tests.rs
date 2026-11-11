use super::*;
use crate::selection::{TimeControl, TimeMode};

#[test]
fn time_layer_keeps_undated_coordinates_out_of_deep_time_and_rotates_plate_assignments() {
    let atlas = crate::catalog::tests::atlas();
    let control = TimeControl {
        mode: TimeMode::MillionYears,
        value: "240".into(),
        ..Default::default()
    };
    let time = SelectedTime::query(&atlas, &control).unwrap();
    let observations = features(&atlas, BodyId::Earth, &time, MapLayer::Observations, None);
    let qimo = EntityId::from_key("earth/cn-gz/zhijin/qimo-commune").unwrap();
    let anchor = observations["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["properties"]["entity_id"] == json!(qimo))
        .unwrap();
    assert_eq!(anchor["properties"]["datum"], "unspecified");
    assert_eq!(anchor["properties"]["resolution_arcminutes"], 1.0);
    for (actual, expected) in anchor["geometry"]["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .zip([105.0 + 47.0 / 60.0, 26.0 + 44.0 / 60.0])
    {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-12);
    }
    // Undated coordinates stay out; the plate-assigned Panxian locality appears
    // at its reconstructed 240 Ma position instead of its present-day anchor.
    let selected = features(&atlas, BodyId::Earth, &time, MapLayer::SelectedTime, None);
    let selected = selected["features"].as_array().unwrap();
    let fauna = EntityId::from_key("earth/cn-gz/panxian/yangjuan-fauna").unwrap();
    assert!(!selected.is_empty());
    assert!(selected.iter().all(|feature| {
        feature["properties"]["reconstructed"] == true
            && feature["properties"]["entity_id"] == json!(fauna)
    }));
    let position = time.paleo.values().next().unwrap();
    assert_eq!(
        selected[0]["properties"]["plate_model"],
        json!(position.model_id)
    );
    let longitude = selected[0]["geometry"]["coordinates"][0].as_f64().unwrap();
    assert!((longitude - position.reconstructed.longitude).abs() < 1e-12);
    assert!((position.observed.longitude - position.reconstructed.longitude).abs() > 1.0);
    let moon = features(&atlas, BodyId::Moon, &time, MapLayer::Observations, None);
    assert_eq!(moon["features"].as_array().unwrap().len(), 3);
    let style = style(
        observations.clone(),
        BodyId::Earth,
        DisplayProjection::Mercator,
    )
    .unwrap();
    assert!(style.validate().is_ok());
    let layers = style
        .layers
        .into_iter()
        .filter(|layer| layer.source.as_deref() == Some("observations"))
        .collect();
    let processed = maplibre::headless::map::process_geojson_layers(
        &observations,
        "observations",
        layers,
        maplibre::coords::WorldTileCoords {
            x: 0,
            y: 0,
            z: maplibre::coords::ZoomLevel::new(0),
        },
        maplibre::projection::ProjectionType::Mercator,
    )
    .unwrap();
    assert!(!processed.indexes.is_empty());
    assert!(!processed.vector.is_empty());
}

#[test]
fn event_layer_passes_through_maplibre_geojson_processing() {
    let atlas = crate::catalog::tests::atlas();
    let overlay = crate::overlay::Overlay::parse(
        &atlas,
        "apollo.json".into(),
        crate::overlay::tests::example(),
    )
    .unwrap();
    let time = SelectedTime::query(
        &atlas,
        &TimeControl {
            mode: TimeMode::Ticks,
            value: "-960910900241".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let markers = overlay.features(&atlas, BodyId::Moon, &time, None).unwrap();
    let collection = json!({"type": "FeatureCollection", "features": markers});
    let style = style(
        collection.clone(),
        BodyId::Moon,
        DisplayProjection::Mercator,
    )
    .unwrap();
    assert!(style.validate().is_ok());
    let layers = style
        .layers
        .into_iter()
        .filter(|layer| layer.id == "external-events")
        .collect();
    let processed = maplibre::headless::map::process_geojson_layers(
        &collection,
        "observations",
        layers,
        maplibre::coords::WorldTileCoords {
            x: 0,
            y: 0,
            z: maplibre::coords::ZoomLevel::new(0),
        },
        maplibre::projection::ProjectionType::Mercator,
    )
    .unwrap();
    assert!(!processed.indexes.is_empty());
    assert!(!processed.vector.is_empty());
}

#[test]
fn time_layer_places_map_registered_entities_without_table_coordinates() {
    let mut atlas = crate::catalog::tests::atlas();
    let control = TimeControl {
        mode: TimeMode::Year,
        value: "1970".into(),
        ..Default::default()
    };
    let time = SelectedTime::query(&atlas, &control).unwrap();
    // Every curated map location also has a table coordinate, so remove one
    // entity's table coordinates to stand for a feature known only from a map.
    let subject = atlas
        .data
        .estimates
        .iter()
        .filter(|estimate| {
            matches!(
                estimate.value,
                atlas_ledger::model::EstimateValue::MapLocation { .. }
            )
        })
        .map(|estimate| estimate.entity_id)
        .find(|id| time.evidence.contains_key(id) && !time.absent.contains(id))
        .unwrap();
    for record in atlas
        .data
        .records
        .iter_mut()
        .filter(|record| record.entity_id == subject)
    {
        record.observation.coordinate = None;
    }
    let selected = features(&atlas, BodyId::Earth, &time, MapLayer::SelectedTime, None);
    let placed: Vec<_> = selected["features"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|feature| feature["properties"]["entity_id"] == json!(subject))
        .collect();
    assert_eq!(placed.len(), 1);
    assert_eq!(
        placed[0]["properties"]["geometry_role"],
        "location_estimate"
    );
}

#[cfg(test)]
fn impact_model() -> atlas_ledger::plates::PlateModelDocument {
    use atlas_core::time::EpochTime;
    use atlas_geo::{
        coordinates::LonLat,
        plates::{PlateId, RotationSample, RotationSequence},
        rotation::EulerRotation,
    };
    let sample = |source_line, age_ma, angle_degrees| RotationSample {
        age_ma,
        rotation: EulerRotation {
            pole: LonLat {
                longitude: 0.0,
                latitude: 90.0,
            },
            angle_degrees,
        },
        source_line,
        description: String::new(),
    };
    atlas_ledger::plates::PlateModelDocument {
        id: "test-plate".into(),
        source_id: "impact-src".into(),
        source_sha256: "0".repeat(64),
        attribution: "Test".into(),
        license: "CC0-1.0".into(),
        reference_frame: "paleomagnetic".into(),
        present_epoch: EpochTime::from_ticks(0),
        youngest_ma: 0.0,
        oldest_ma: 100.0,
        sequences: vec![RotationSequence {
            moving_plate: PlateId(101),
            fixed_plate: PlateId(0),
            samples: vec![sample(1, 0.0, 0.0), sample(2, 100.0, 20.0)],
        }],
    }
}

#[cfg(test)]
fn impact_atlas() -> (crate::catalog::Atlas, EntityId) {
    use atlas_core::{
        identity::FactId,
        origin::OriginTime,
        time::{EpochTime, InstantEstimate, TimePrecision},
    };
    use atlas_geo::plates::PlateId;
    use atlas_ledger::model::{Entity, Estimate, EstimateValue, Record};

    let impact = EntityId::from_key("earth/test/impact-crater").unwrap();
    let rec_id = FactId::from_key("survey/impact-crater").unwrap();
    let at_66_ma = EpochTime::years_before(EpochTime::from_ticks(0), 66_000_000).unwrap();
    let source = serde_json::from_value(json!({
        "id": "impact-src", "title": "Test impact monograph",
        "scan_pages": 1, "scan_sha256": "0".repeat(64)
    }))
    .unwrap();
    let entity: Entity = serde_json::from_value(json!({
        "id": impact, "key": "earth/test/impact-crater", "body": "earth",
        "kind": {"geology": "crater"}
    }))
    .unwrap();
    let record: Record = serde_json::from_value(json!({
        "id": rec_id, "key": "survey/impact-crater", "entity_id": impact, "source_id": "impact-src",
        "scan_page": 1, "page_or_sheet": "1", "locator": "Table 1", "checked": true,
        "observation": {"name": "Test Impact", "coordinate": {
            "point": {"longitude": -89.5, "latitude": 21.4}, "datum": "wgs84",
            "longitude_text": "89°30′W", "latitude_text": "21°24′N", "resolution_arcminutes": 1.0}}
    }))
    .unwrap();
    let origin = Estimate {
        id: FactId::from_key("estimate/impact-origin").unwrap(),
        key: "estimate/impact-origin".into(),
        entity_id: impact,
        basis: vec![rec_id],
        brief: "Radiometric impact age.".into(),
        value: EstimateValue::Origin {
            time: OriginTime::Estimated {
                instant: InstantEstimate {
                    earliest: at_66_ma,
                    nominal: at_66_ma,
                    latest: at_66_ma,
                    precision: TimePrecision::Ma,
                },
            },
        },
    };
    let plate = Estimate {
        id: FactId::from_key("estimate/impact-plate").unwrap(),
        key: "estimate/impact-plate".into(),
        entity_id: impact,
        basis: vec![rec_id],
        brief: "Host continental block.".into(),
        value: EstimateValue::Plate {
            model_id: "test-plate".into(),
            plate_id: PlateId(101),
            validity: None,
        },
    };
    let entry = crate::catalog::Entry {
        geometry: None,
        lines: Vec::new(),
        derived_boundaries: Vec::new(),
        boundaries: Vec::new(),
        drawings: Vec::new(),
        entity,
        records: vec![record],
        estimates: vec![origin, plate],
        disputes: Vec::new(),
    };
    let atlas = crate::catalog::Atlas::new(crate::catalog::Publication {
        sources: vec![source],
        entries: std::collections::BTreeMap::from([(impact, entry)]),
        aliases: Default::default(),
        plate_models: std::collections::BTreeMap::from([("test-plate".into(), impact_model())]),
    })
    .unwrap();
    (atlas, impact)
}

#[test]
fn pre_origin_impact_stays_hidden_until_crustal_pins_are_enabled() {
    let (atlas, _impact) = impact_atlas();
    let before = SelectedTime::query(
        &atlas,
        &TimeControl {
            mode: TimeMode::MillionYears,
            value: "70".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(before.paleo.is_empty());
    assert_eq!(before.crustal_pins.len(), 1);
    assert!(
        features_with_pins(
            &atlas,
            BodyId::Earth,
            &before,
            MapLayer::SelectedTime,
            None,
            false
        )["features"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let pinned = features_with_pins(
        &atlas,
        BodyId::Earth,
        &before,
        MapLayer::SelectedTime,
        None,
        true,
    );
    let pin_features = pinned["features"].as_array().unwrap();
    assert_eq!(pin_features.len(), 1);
    assert_eq!(
        pin_features[0]["properties"]["geometry_role"],
        "crustal_pin"
    );
    assert_eq!(pin_features[0]["properties"]["unformed"], true);
    assert!(
        style(pinned, BodyId::Earth, DisplayProjection::Mercator)
            .unwrap()
            .validate()
            .is_ok()
    );

    let after = SelectedTime::query(
        &atlas,
        &TimeControl {
            mode: TimeMode::MillionYears,
            value: "65".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(after.paleo.len(), 1);
    let extant = features_with_pins(
        &atlas,
        BodyId::Earth,
        &after,
        MapLayer::SelectedTime,
        None,
        false,
    );
    assert_eq!(extant["features"].as_array().unwrap().len(), 1);
    assert_eq!(
        extant["features"][0]["properties"]["geometry_role"],
        "source_observation"
    );
}
