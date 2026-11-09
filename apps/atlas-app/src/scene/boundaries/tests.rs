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
pub(crate) fn lunar_publication() -> Publication {
    use atlas_core::identity::FactId;
    let id = EntityId::from_key("moon/test/enclosure").unwrap();
    let record_id = FactId::from_key("survey/enclosure").unwrap();
    let source = json!({"id":"survey","title":"Test lunar survey","scan_pages":1,"scan_sha256":"0".repeat(64)});
    let record = json!({"id":record_id,"key":"survey/enclosure","entity_id":id,"source_id":"survey",
        "scan_page":1,"page_or_sheet":"A","locator":"Enclosure","checked":true,"observation":{"name":"Enclosure"}});
    let boundary = json!({"unit":{"record_id":record_id,"polygons":[{"exterior":[0,1,2,3,0],"interiors":[]}],
        "validity":{"start":{"earliest":"10","nominal":"12","latest":"14","precision":"day"},
            "end":{"earliest":"20","nominal":"20","latest":"20","precision":"day"}}},
        "vertices":[{"x":1,"y":1},{"x":2,"y":1},{"x":2,"y":2},{"x":1,"y":2}],
        "datum":"moon_iau","boundary_method":"unstated","method_basis":[],"registration":"maps/survey.json"});
    let entry: Entry = serde_json::from_value(
        json!({"entity":{"id":id,"key":"moon/test/enclosure","body":"moon","kind":"facility"},
        "records":[record],"estimates":[],"disputes":[],"boundaries":[boundary]}),
    )
    .unwrap();
    Publication {
        sources: serde_json::from_value(json!([source])).unwrap(),
        entries: BTreeMap::from([(id, entry)]),
        aliases: Default::default(),
    }
}

#[test]
fn dated_lunar_boundaries_follow_uncertainty_and_exclusive_endpoints() {
    use crate::selection::{SelectedTime, TimeControl, TimeMode};
    use atlas_ledger::timeline::TimeSelection;
    let atlas = Atlas::new(lunar_publication()).unwrap();
    for (ticks, mode, expected) in [
        (9, TimeSelection::Possible, None),
        (10, TimeSelection::Possible, Some("possible")),
        (12, TimeSelection::Certain, None),
        (14, TimeSelection::Certain, Some("certain")),
        (20, TimeSelection::Possible, None),
    ] {
        let time = SelectedTime::query(
            &atlas,
            &TimeControl {
                mode: TimeMode::Ticks,
                value: ticks.to_string(),
                selection: mode,
            },
        )
        .unwrap();
        let selected = features(&atlas, BodyId::Moon, Some(&time), None, false).unwrap();
        assert_eq!(selected.len(), usize::from(expected.is_some()));
        if let Some(expected) = expected {
            assert_eq!(selected[0]["properties"]["membership"], expected);
        }
        assert!(
            features(&atlas, BodyId::Earth, Some(&time), None, false)
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        features(&atlas, BodyId::Moon, None, None, false)
            .unwrap()
            .len(),
        1
    );
}

#[cfg(test)]
fn compiled() -> Artifacts {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = atlas_compile::input::load_blocking(&root).unwrap();
    input.derivations.clear();
    let trace: SheetTrace = serde_json::from_slice(
        &std::fs::read(root.join("guizhou-1960s/zhijin/ertang.trace.json")).unwrap(),
    )
    .unwrap();
    let control = |name: &str, x: f64, y: f64| {
        json!({
            "landmark": name, "reference": "Synthetic affine frame for publication tests",
            "reference_sha256": "0".repeat(64),
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
    let mut entries: BTreeMap<EntityId, Entry> = output
        .iter()
        .filter(|(p, _)| p.starts_with("entities/index"))
        .flat_map(|(_, bytes)| serde_json::from_slice::<BTreeMap<EntityId, Entry>>(bytes).unwrap())
        .collect();
    for entry in entries.values_mut() {
        if let Some(reference) = &entry.geometry {
            let parts = reference
                .parts
                .iter()
                .map(|part| output[&std::path::PathBuf::from(&part.path)].clone())
                .collect::<Vec<_>>();
            entry.attach_geometry(reference.decode(&parts).unwrap());
        }
    }
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
    let boundaries = features(&atlas, BodyId::Earth, None, Some(id), true).unwrap();
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
        features(&atlas, BodyId::Moon, None, Some(id), true)
            .unwrap()
            .is_empty()
    );
    assert!(
        features(
            &atlas,
            BodyId::Earth,
            Some(
                &crate::selection::SelectedTime::query(
                    &atlas,
                    &crate::selection::TimeControl::default()
                )
                .unwrap()
            ),
            Some(id),
            true
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        features(&atlas, BodyId::Earth, None, None, false)
            .unwrap()
            .iter()
            .filter(|f| f["properties"]["entity_id"] == json!(id))
            .count(),
        1
    );
    assert!(center(&atlas, id, false, None).unwrap().validate().is_ok());
}

#[test]
fn mapped_polygons_tessellate_fill_outline_and_selection_for_both_projections() {
    let atlas = Atlas::new(publication()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/zhijin/ertang-commune").unwrap();
    let collection = json!({"type": "FeatureCollection", "features": features(&atlas, BodyId::Earth, None, Some(id), true).unwrap()});
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
                if name.ends_with("selection") {
                    "selection"
                } else {
                    "observations"
                },
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
    let atlas = Atlas::new(crate::catalog::tests::terrain_publication()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    let boundaries = features(&atlas, BodyId::Earth, None, Some(id), true).unwrap();
    assert_eq!(
        boundaries
            .iter()
            .filter(|f| f["properties"]["reconstructed"] == true)
            .count(),
        124
    );
    assert!(frame(&atlas, id, [800., 600.], true, None).is_some());
    assert!(
        features(
            &atlas,
            BodyId::Earth,
            Some(
                &crate::selection::SelectedTime::query(
                    &atlas,
                    &crate::selection::TimeControl::default()
                )
                .unwrap()
            ),
            Some(id),
            true
        )
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
                if name.ends_with("selection") {
                    "selection"
                } else {
                    "observations"
                },
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
