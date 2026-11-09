use super::*;
use crate::registration::{SYNTHETIC_REFERENCE, with_synthetic_reference};

use atlas_core::identity::{EntityId, FactId};
use atlas_geo::registration::ControlPoint;
use atlas_geo::registration::PlanarPoint;
use serde_json::{Value, json};
use std::collections::BTreeMap;

mod rejection;

#[cfg(test)]
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

#[cfg(test)]
fn trace() -> SheetTrace {
    read_json_blocking(&root().join("guizhou-1960s/zhijin/ertang.trace.json")).unwrap()
}

#[cfg(test)]
fn controls() -> BoundaryRegistration {
    let control = |name: &str, x: f64, y: f64| ReferencedControl {
        basis: Vec::new(),
        landmark: name.into(),
        reference: "synthetic registration test frame".into(),
        reference_sha256: Some(SYNTHETIC_REFERENCE.into()),
        uncertainty: None,
        point: ControlPoint {
            source: PlanarPoint { x, y },
            target: PlanarPoint {
                x: 105. + x / 10_000.,
                y: 27. - y / 10_000.,
            },
        },
    };
    BoundaryRegistration {
        target_frame: TargetFrame {
            datum: None,
            crs: "OGC:CRS84".into(),
            units: "degrees".into(),
        },
        method: RegistrationMethod::Affine,
        controls: vec![
            control("a", 0., 0.),
            control("b", 1000., 0.),
            control("c", 0., 1500.),
        ],
        check_points: vec![control("holdout", 900., 1400.)],
        maximum_segment_pixels: 10.,
        terrain: None,
    }
}

#[cfg(test)]
fn json(artifacts: &Artifacts, path: &str) -> Value {
    serde_json::from_slice(&artifacts[Path::new(path)]).unwrap()
}

#[test]
fn reviewed_ertang_trace_keeps_pixel_frame_and_source_boundary_method() {
    let mut input = with_synthetic_reference(load_blocking(&root()).unwrap());
    input.derivations.clear();
    let trace = trace();
    let artifacts = compile_trace(&input, trace.clone()).unwrap();
    let manifest = json(&artifacts, "boundary-trace.json");
    assert_eq!(manifest["geometry_kind"], "map_trace");
    assert_eq!(manifest["boundary_method"], "compiled_from_places");
    assert_eq!(manifest["source_frame"]["width"], 1046);
    assert_eq!(manifest["vertex_count"], 121);
    assert!(!artifacts.contains_key(Path::new("boundary.json")));
    let vertices: Vec<PlanarPoint> =
        serde_json::from_value(json(&artifacts, manifest["vertices"][0].as_str().unwrap()))
            .unwrap();
    assert_eq!(vertices, trace.vertices);
    let method = evidence::record(&input, trace.method_basis[0]).unwrap();
    assert_eq!(method.scan_page, Some(4));
    assert!(
        method
            .verbatim_quote
            .as_deref()
            .unwrap()
            .contains("均未进行实测")
    );
    assert!(
        artifacts
            .values()
            .all(|bytes| bytes.len() < crate::export::MAX_FILE_BYTES)
    );
}

#[test]
fn registration_preserves_provenance_and_reflected_rings_in_geographic_output() {
    let mut input = with_synthetic_reference(load_blocking(&root()).unwrap());
    input.derivations.clear();
    let request = trace();
    let artifacts = compile_boundary(&input, request.clone(), controls()).unwrap();
    let manifest = json(&artifacts, "boundary.json");
    assert_eq!(manifest["geometry_kind"], "mapped_polygon");
    assert!(manifest["vertex_count"].as_u64().unwrap() > 121);
    let proof = json(&artifacts, "boundary/registration.json");
    assert!(proof["control_quality"]["rms"].as_f64().unwrap() < 1e-12);
    assert!(proof["check_quality"]["maximum"].as_f64().unwrap() < 1e-12);
    assert_eq!(proof["maximum_segment_pixels"], 10.);
    let geojson = json(&artifacts, manifest["features"][0].as_str().unwrap());
    let feature = &geojson["features"][0];
    assert_eq!(
        feature["properties"]["boundary_method"],
        "compiled_from_places"
    );
    assert_eq!(
        feature["properties"]["record_id"],
        request.units[0].record_id.0.to_string()
    );
    assert!(feature["properties"]["validity"].is_null());
    let ring = feature["geometry"]["coordinates"][0][0].as_array().unwrap();
    let area: f64 = ring
        .windows(2)
        .map(|pair| {
            pair[0][0].as_f64().unwrap() * pair[1][1].as_f64().unwrap()
                - pair[1][0].as_f64().unwrap() * pair[0][1].as_f64().unwrap()
        })
        .sum();
    assert!(area > 0.);
    assert_eq!(ring.first(), ring.last());
}

#[test]
fn projected_boundary_preserves_crs_and_does_not_emit_projected_geojson() {
    let mut input = with_synthetic_reference(load_blocking(&root()).unwrap());
    input.derivations.clear();
    let mut parameters = controls();
    parameters.target_frame = TargetFrame {
        datum: None,
        crs: "EPSG:32648".into(),
        units: "metres".into(),
    };
    for control in parameters
        .controls
        .iter_mut()
        .chain(&mut parameters.check_points)
    {
        control.point.target = PlanarPoint {
            x: 400_000. + control.point.source.x * 2.,
            y: 3_000_000. - control.point.source.y * 2.,
        };
    }
    let artifacts = compile_boundary(&input, trace(), parameters).unwrap();
    let manifest = json(&artifacts, "boundary.json");
    assert_eq!(manifest["target_frame"]["crs"], "EPSG:32648");
    assert_eq!(manifest["features"], serde_json::json!([]));
    assert_eq!(
        json(&artifacts, manifest["vertices"][0].as_str().unwrap())[0]["x"],
        400406.
    );
}

#[test]
fn boundary_command_publishes_both_modes_and_refuses_existing_destinations() {
    let temp = tempfile::tempdir().unwrap();
    let path = root().join("guizhou-1960s/zhijin/ertang.trace.json");
    let output = temp.path().join("trace");
    assert!(boundary_blocking(&root(), &path, None, &output).unwrap() > 200);
    assert!(boundary_blocking(&root(), &path, None, &output).is_err());
    let parameters = temp.path().join("controls.json");
    // The command reloads the ledger from disk, so its controls cite the trace's checked method fact.
    let method_fact = trace().method_basis[0];
    let mut cited = controls();
    for control in cited.controls.iter_mut().chain(&mut cited.check_points) {
        control.reference_sha256 = None;
        control.basis.push(method_fact);
    }
    std::fs::write(&parameters, serde_json::to_vec(&cited).unwrap()).unwrap();
    let registered = temp.path().join("registered");
    boundary_blocking(&root(), &path, Some(&parameters), &registered).unwrap();
    assert!(registered.join("boundary.json").is_file());
    assert!(registered.join("boundary-trace.json").is_file());
}

#[test]
fn unpaginated_line_only_capture_registers_and_publishes_open_components() {
    let entity = EntityId::from_key("earth/test/river").unwrap();
    let fact = FactId::from_key("capture/river").unwrap();
    let mut input = with_synthetic_reference(LedgerInput {
        catalog: serde_json::from_value(json!({"sources":[{"id":"capture","title":"Test frame",
            "capture":{"medium":"aerial_photograph","sha256":"a".repeat(64),"frame":"17"}}]})).unwrap(),
        data: serde_json::from_value(json!({"entities":[{"id":entity,"key":"earth/test/river","body":"earth","kind":"river"}],
            "records":[{"id":fact,"key":"capture/river","entity_id":entity,"source_id":"capture","locator":"River","checked":true,"observation":{"name":"River"}}],"estimates":[],"disputes":[]})).unwrap(),
        aliases: Default::default(), published: Default::default(), derivations: BTreeMap::new(),
        traces: Vec::new(), place_maps: BTreeMap::new(), plate_models: BTreeMap::new(),
    });
    let trace: SheetTrace = serde_json::from_value(json!({"source_id":"capture","scan_page":null,
        "source_frame":{"width":1000,"height":1500},"boundary_method":"unstated","method_basis":[],
        "vertices":[{"x":100,"y":100},{"x":400,"y":100},{"x":600,"y":100},{"x":900,"y":100}],
        "lines":[{"record_id":fact,"paths":[[0,1],[2,3]],"validity":{"start":{"earliest":"0","nominal":"0","latest":"0","precision":"day"},"end":null}}]})).unwrap();
    let mut parameters = controls();
    parameters.method = RegistrationMethod::ThinPlate { smoothing: 0.0 };
    for control in &mut parameters.controls {
        control.basis.push(fact);
    }
    let artifacts = compile_boundary(&input, trace.clone(), parameters.clone()).unwrap();
    let manifest = json(&artifacts, "boundary.json");
    let feature = &json(&artifacts, manifest["features"][0].as_str().unwrap())["features"][0];
    assert_eq!(feature["geometry"]["type"], "MultiLineString");
    assert_eq!(feature["properties"]["record_id"], json!(fact));
    assert_eq!(feature["properties"]["validity"]["start"]["nominal"], "0");
    let paths = feature["geometry"]["coordinates"].as_array().unwrap();
    assert_eq!(paths.len(), 2);
    assert!(
        paths
            .iter()
            .all(|path| path.as_array().unwrap().len() == 31)
    );
    assert_ne!(paths[0][0], paths[0][30]);
    let shard = json(
        &artifacts,
        &format!("entities/index/{}.json", entity.shard()),
    );
    assert_eq!(
        crate::export::resolve_geometry(&artifacts, &shard[entity.0.to_string()]).unwrap()["lines"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    input.data.entities[0].body = atlas_geo::body::BodyId::Moon;
    parameters.target_frame.crs = "IAU:Moon".into();
    parameters.target_frame.datum = Some(atlas_geo::coordinates::Datum::MoonIau);
    let lunar = compile_boundary(&input, trace.clone(), parameters.clone()).unwrap();
    let lunar_manifest = json(&lunar, "boundary.json");
    let lunar_features = json(&lunar, lunar_manifest["features"][0].as_str().unwrap());
    assert_eq!(
        lunar_features["features"][0]["properties"]["datum"],
        "moon_iau"
    );
    input.data.records[0].checked = false;
    assert!(compile_boundary(&input, trace, parameters).is_err());
}
