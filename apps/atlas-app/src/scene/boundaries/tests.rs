use super::*;
use crate::catalog::{Entry, Publication};
use atlas_compile::{
    boundary::{BoundaryRegistration, SheetTrace, compile_boundary},
    export::{Artifacts, write_blocking},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[cfg(test)]
fn compiled() -> Artifacts {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = atlas_compile::input::load_blocking(&root).unwrap();
    let trace: SheetTrace = serde_json::from_slice(
        &std::fs::read(root.join("guizhou-1960s/zhijin/ertang.trace.json")).unwrap(),
    )
    .unwrap();
    let control = |name: &str, x: f64, y: f64| {
        json!({
            "landmark": name, "reference": "Synthetic affine frame for publication tests",
            "point": {"source": {"x": x, "y": y},
                "target": {"x": 105. + x / 10000., "y": 27. - y / 10000.}}
        })
    };
    let parameters: BoundaryRegistration = serde_json::from_value(json!({
        "target_frame": {"crs": "OGC:CRS84", "units": "degrees"},
        "method": {"kind": "affine"},
        "controls": [control("a", 0., 0.), control("b", 1000., 0.), control("c", 0., 1500.)],
        "check_points": [control("holdout", 900., 1400.)], "maximum_segment_pixels": 10.
    }))
    .unwrap();
    compile_boundary(&input, trace, parameters).unwrap()
}

#[cfg(test)]
pub(crate) fn publication() -> Publication {
    let output = compiled();
    let sources = serde_json::from_slice(&output[&PathBuf::from("sources.json")]).unwrap();
    let entries = output
        .iter()
        .filter(|(p, _)| p.starts_with("entities/index"))
        .flat_map(|(_, bytes)| serde_json::from_slice::<BTreeMap<EntityId, Entry>>(bytes).unwrap())
        .collect();
    Publication {
        sources,
        entries,
        aliases: Default::default(),
    }
}

#[test]
fn registered_boundary_loads_with_source_evidence_and_stays_out_of_time_slices() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("atlas");
    let artifacts = compiled();
    write_blocking(&path, &artifacts).unwrap();
    let atlas = crate::publication::load_blocking(&path).unwrap();
    let id = EntityId::from_key("earth/cn-gz/zhijin/ertang-commune").unwrap();
    let boundaries = features(&atlas, BodyId::Earth, MapLayer::Observations, Some(id)).unwrap();
    let feature = boundaries
        .iter()
        .find(|f| f["properties"]["entity_id"] == json!(id))
        .unwrap();
    assert_eq!(feature["properties"]["entity_id"], json!(id));
    assert_eq!(feature["properties"]["page_or_sheet"], "32");
    assert_eq!(feature["properties"]["scan_page"], 40);
    assert_eq!(feature["properties"]["geometry_kind"], "mapped_polygon");
    assert_eq!(
        feature["properties"]["boundary_method"],
        "compiled_from_places"
    );
    assert_eq!(feature["properties"]["selected"], true);
    assert_eq!(feature["properties"]["datum"], "wgs84");
    assert!(feature["properties"]["registration_allowance_m"].is_null());
    assert!(feature["properties"]["validity"].is_null());
    assert_eq!(
        feature["properties"]["registration"],
        "boundary/registration.json"
    );
    let proof: Value =
        serde_json::from_slice(&artifacts[Path::new("boundary/registration.json")]).unwrap();
    assert!(proof["check_quality"]["maximum"].as_f64().unwrap() < 1e-12);
    assert!(
        features(&atlas, BodyId::Moon, MapLayer::Observations, Some(id))
            .unwrap()
            .is_empty()
    );
    assert!(
        features(&atlas, BodyId::Earth, MapLayer::SelectedTime, Some(id))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        features(&atlas, BodyId::Earth, MapLayer::MappedPlaces, None)
            .unwrap()
            .iter()
            .filter(|f| f["properties"]["entity_id"] == json!(id))
            .count(),
        1
    );
    assert!(center(&atlas, id).unwrap().validate().is_ok());
}

#[test]
fn mapped_polygons_tessellate_fill_outline_and_selection_for_both_projections() {
    let atlas = Atlas::new(publication()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/zhijin/ertang-commune").unwrap();
    let collection = json!({"type": "FeatureCollection", "features": features(&atlas, BodyId::Earth, MapLayer::Observations, Some(id)).unwrap()});
    for projection in [
        maplibre::projection::ProjectionType::Mercator,
        maplibre::projection::ProjectionType::Globe,
    ] {
        let style =
            crate::scene::style(collection.clone(), BodyId::Earth, projection.clone()).unwrap();
        assert!(style.validate().is_ok());
        for name in ["boundaries", "boundary-lines", "boundary-selection"] {
            let layers = style
                .layers
                .iter()
                .filter(|layer| layer.id == name)
                .cloned()
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
                projection.clone(),
            )
            .unwrap();
            assert!(!processed.indexes.is_empty(), "{name}");
            assert!(
                processed
                    .vector
                    .iter()
                    .any(|layer| layer.buffer.usable_indices > 0),
                "{name}"
            );
        }
    }
}

#[test]
fn reconstructed_communes_load_frame_and_tessellate_with_dashed_outlines() {
    let atlas = Atlas::new(crate::catalog::tests::publication()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    let boundaries = features(&atlas, BodyId::Earth, MapLayer::Observations, Some(id)).unwrap();
    assert_eq!(
        boundaries
            .iter()
            .filter(|f| f["properties"]["reconstructed"] == true)
            .count(),
        124
    );
    assert!(frame(&atlas, id, [800., 600.]).is_some());
    assert!(
        features(&atlas, BodyId::Earth, MapLayer::SelectedTime, Some(id))
            .unwrap()
            .is_empty()
    );
    let collection = json!({"type":"FeatureCollection","features":boundaries});
    for projection in [
        maplibre::projection::ProjectionType::Mercator,
        maplibre::projection::ProjectionType::Globe,
    ] {
        let style =
            crate::scene::style(collection.clone(), BodyId::Earth, projection.clone()).unwrap();
        assert!(style.validate().is_ok());
        for name in ["derived-boundaries", "derived-lines", "derived-selection"] {
            let layers = style
                .layers
                .iter()
                .filter(|l| l.id == name)
                .cloned()
                .collect();
            let rendered = maplibre::headless::map::process_geojson_layers(
                &collection,
                "observations",
                layers,
                maplibre::coords::WorldTileCoords {
                    x: 0,
                    y: 0,
                    z: maplibre::coords::ZoomLevel::new(0),
                },
                projection.clone(),
            )
            .unwrap();
            assert!(!rendered.indexes.is_empty(), "{name}");
            assert!(
                rendered.vector.iter().any(|l| l.buffer.usable_indices > 0),
                "{name}"
            );
        }
    }
}

#[test]
fn publication_rejects_misfiled_boundary_and_invalid_geometry_or_method() {
    let id = EntityId::from_key("earth/cn-gz/zhijin/ertang-commune").unwrap();
    let mut misplaced = publication();
    let boundary = misplaced
        .entries
        .get_mut(&id)
        .unwrap()
        .boundaries
        .pop()
        .unwrap();
    misplaced
        .entries
        .values_mut()
        .find(|e| e.entity.id != id)
        .unwrap()
        .boundaries
        .push(boundary);
    assert!(matches!(
        Atlas::new(misplaced),
        Err(AppError::EvidenceIdentity(_))
    ));
    for case in 0..9 {
        let mut input = publication();
        let boundary = &mut input.entries.get_mut(&id).unwrap().boundaries[0];
        match case {
            0 => boundary.vertices[0].x = 181.,
            1 => boundary.unit.polygons[0]
                .exterior
                .pop()
                .map(|_| ())
                .unwrap(),
            2 => boundary.method_basis.clear(),
            3 => boundary.registration = "../outside.json".into(),
            4 => boundary.datum = atlas_geo::coordinates::Datum::MoonIau,
            5 => boundary.registration_allowance_m = Some(-1.),
            6 => boundary.registration_allowance_m = Some(f64::NAN),
            7 => boundary.registration_allowance_m = Some(f64::INFINITY),
            _ => boundary.unit.polygons[0].exterior[1] = usize::MAX,
        }
        assert!(
            matches!(Atlas::new(input), Err(AppError::Boundary(_))),
            "case {case}"
        );
    }
}
