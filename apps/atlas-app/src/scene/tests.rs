use super::*;
use crate::selection::{TimeControl, TimeMode};

#[test]
fn time_layer_keeps_undated_coordinates_out_of_deep_time() {
    let atlas = crate::catalog::tests::atlas();
    let control = TimeControl {
        mode: TimeMode::MillionYears,
        value: "240".into(),
        ..Default::default()
    };
    let time = SelectedTime::query(&atlas, &control).unwrap();
    let observations = features(&atlas, BodyId::Earth, &time, MapLayer::Observations, None);
    assert_eq!(observations["features"].as_array().unwrap().len(), 1869);
    let selected = features(&atlas, BodyId::Earth, &time, MapLayer::SelectedTime, None);
    assert!(selected["features"].as_array().unwrap().is_empty());
    let moon = features(&atlas, BodyId::Moon, &time, MapLayer::Observations, None);
    assert_eq!(moon["features"].as_array().unwrap().len(), 3);
    let style = style(observations.clone()).unwrap();
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
    let style = style(collection.clone()).unwrap();
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
