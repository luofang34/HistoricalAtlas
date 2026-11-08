use super::*;
use crate::{export, input::load_blocking};
use atlas_boundary::{
    polygon::IndexedPolygon,
    sheet::{BoundaryMethod, SheetTrace, TracedUnit},
};
use atlas_geo::{coordinates::Datum, registration::PlanarPoint};
use atlas_ledger::model::EstimateValue;
use std::path::Path;

#[cfg(test)]
fn input() -> LedgerInput {
    let mut input =
        load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let map = &input.place_maps["bijie-town"];
    let record = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/town-map/wuliping-commune")
        .unwrap();
    let point = map
        .places
        .iter()
        .find(|p| p.record_id == record.id)
        .unwrap()
        .point;
    // A synthetic rectangle exercises registration with the reviewed Bijie controls.
    input.traces.push(SheetTrace {
        source_id: map.source_id.clone(),
        scan_page: map.scan_page,
        source_frame: map.source_frame,
        boundary_method: BoundaryMethod::Unstated,
        method_basis: Vec::new(),
        places: Vec::new(),
        vertices: vec![
            point,
            PlanarPoint {
                x: point.x + 50.,
                y: point.y,
            },
            PlanarPoint {
                x: point.x + 50.,
                y: point.y + 50.,
            },
            PlanarPoint {
                x: point.x,
                y: point.y + 50.,
            },
        ],
        units: vec![TracedUnit {
            record_id: record.id,
            polygons: vec![IndexedPolygon {
                exterior: vec![0, 1, 2, 3, 0],
                interiors: Vec::new(),
            }],
        }],
    });
    input
}

#[test]
fn ordinary_build_registers_sheet_boundaries_with_the_same_place_controls_and_datum() {
    let input = input();
    let output = export::compile(&input).unwrap();
    let record_id = input.traces.last().unwrap().units[0].record_id;
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|e| {
            matches!(e.value,
        EstimateValue::MapLocation {record_id: id, ..} if id == record_id)
        })
        .unwrap();
    let path = format!("entities/index/{}.json", estimate.entity_id.shard());
    let shard: serde_json::Value = serde_json::from_slice(&output[Path::new(&path)]).unwrap();
    let boundaries = shard[estimate.entity_id.0.to_string()]["boundaries"]
        .as_array()
        .unwrap();
    assert_eq!(boundaries.len(), 1);
    let boundary: GeographicBoundary = serde_json::from_value(boundaries[0].clone()).unwrap();
    assert_eq!(boundary.datum, Datum::Unspecified);
    assert_eq!(boundary.registration_allowance_m, Some(4500.));
    assert_eq!(boundary.registration, "maps/bijie-town.json");
    let EstimateValue::MapLocation { point, .. } = estimate.value else {
        panic!("map location expected")
    };
    assert!((boundary.vertices[0].x - point.longitude).abs() < 1e-12);
    assert!((boundary.vertices[0].y - point.latitude).abs() < 1e-12);
    let feature = boundary.feature(&input.data, None).unwrap();
    assert_eq!(feature["properties"]["datum"], "unspecified");
    assert_eq!(feature["properties"]["registration_allowance_m"], 4500.);
    assert_eq!(feature["properties"]["scan_page"], 22);
    assert_eq!(feature["properties"]["boundary_method"], "unstated");
    let proof: serde_json::Value =
        serde_json::from_slice(&output[Path::new("maps/bijie-town.json")]).unwrap();
    assert_eq!(proof["registration"]["check_quality"]["points"], 25);
    assert_eq!(
        proof["registration"]["job"]["target_frame"]["crs"],
        "ATLAS:source-angular:Unspecified"
    );
}

#[test]
fn different_pixel_frames_and_ambiguous_registrations_are_rejected() {
    let mut input = input();
    input.traces.last_mut().unwrap().source_frame.width = 1700;
    assert!(
        export::compile(&input)
            .unwrap_err()
            .to_string()
            .contains("raster dimensions")
    );
    input.traces.last_mut().unwrap().source_frame.width = 1648;
    let mut alternative = input.place_maps["bijie-town"].clone();
    alternative.id = "alternative".into();
    input.place_maps.insert(alternative.id.clone(), alternative);
    assert!(
        export::compile(&input)
            .unwrap_err()
            .to_string()
            .contains("multiple place-map")
    );
    input.place_maps.clear();
    assert!(register_boundaries(&input).unwrap().is_empty());
}

#[test]
fn explicit_boundary_registration_replaces_the_automatic_fit_for_its_record() {
    let input = input();
    let mut explicit = register_boundaries(&input).unwrap().remove(0);
    explicit.datum = Datum::Wgs84;
    explicit.registration_allowance_m = None;
    explicit.registration = "boundary/registration.json".into();
    for point in &mut explicit.vertices {
        point.x += 1.;
    }
    let output = export::compile_with_boundaries(&input, &[explicit.clone()]).unwrap();
    let record = input
        .data
        .records
        .iter()
        .find(|r| r.id == explicit.unit.record_id)
        .unwrap();
    let path = format!("entities/index/{}.json", record.entity_id.shard());
    let shard: serde_json::Value = serde_json::from_slice(&output[Path::new(&path)]).unwrap();
    let published = shard[record.entity_id.0.to_string()]["boundaries"]
        .as_array()
        .unwrap();
    assert_eq!(published.len(), 1);
    let published: GeographicBoundary = serde_json::from_value(published[0].clone()).unwrap();
    assert_eq!(published.unit.record_id, explicit.unit.record_id);
    assert_eq!(published.datum, Datum::Wgs84);
    assert_eq!(published.registration, "boundary/registration.json");
    assert_eq!(published.registration_allowance_m, None);
    assert_eq!(published.vertices.len(), explicit.vertices.len());
    for (actual, expected) in published.vertices.iter().zip(&explicit.vertices) {
        assert!((actual.x - expected.x).abs() < 1e-12);
        assert!((actual.y - expected.y).abs() < 1e-12);
    }
}

#[test]
fn county_outline_publishes_closed_geometry_containing_its_mapped_places() {
    use atlas_core::identity::EntityId;

    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let output = export::compile(&input).unwrap();
    let id = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    let path = format!("entities/index/{}.json", id.shard());
    let shard: serde_json::Value = serde_json::from_slice(&output[Path::new(&path)]).unwrap();
    let boundary: GeographicBoundary =
        serde_json::from_value(shard[id.0.to_string()]["boundaries"][0].clone()).unwrap();
    let feature = boundary.feature(&input.data, None).unwrap();
    assert_eq!(feature["properties"]["geometry_kind"], "mapped_polygon");
    assert_eq!(feature["properties"]["scan_page"], 658);
    assert_eq!(feature["properties"]["boundary_method"], "unstated");
    assert!(feature["properties"]["validity"].is_null());
    assert_eq!(boundary.datum, Datum::Unspecified);
    assert_eq!(boundary.registration_allowance_m, Some(3500.0));
    assert_eq!(boundary.registration, "maps/bijie-county.json");
    let polygon = &boundary.unit.polygons[0];
    assert!(polygon.interiors.is_empty());
    assert_eq!(polygon.exterior.first(), polygon.exterior.last());
    let ring: Vec<_> = polygon
        .exterior
        .iter()
        .map(|i| boundary.vertices[*i])
        .collect();
    assert!(
        ring.iter()
            .all(|p| (104.8..106.0).contains(&p.x) && (27.0..27.9).contains(&p.y))
    );
    let (_, locations) = register_places(&input, &input.place_maps["bijie-county"]).unwrap();
    for location in locations {
        let EstimateValue::MapLocation { point, .. } = location.value else {
            panic!("registered place expected")
        };
        assert!(
            contains(
                &ring,
                PlanarPoint {
                    x: point.longitude,
                    y: point.latitude
                }
            ),
            "{} lies outside the county outline",
            location.key
        );
    }
}

#[cfg(test)]
fn contains(ring: &[PlanarPoint], point: PlanarPoint) -> bool {
    ring.windows(2)
        .filter(|edge| {
            let (a, b) = (edge[0], edge[1]);
            (a.y > point.y) != (b.y > point.y)
                && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        })
        .count()
        % 2
        == 1
}
