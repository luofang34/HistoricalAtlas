use super::*;
use atlas_geo::registration::ControlPoint;
use serde_json::Value;

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
        landmark: name.into(),
        reference: "synthetic registration test frame".into(),
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
    }
}

#[cfg(test)]
fn json(artifacts: &Artifacts, path: &str) -> Value {
    serde_json::from_slice(&artifacts[Path::new(path)]).unwrap()
}

#[test]
fn reviewed_ertang_trace_keeps_pixel_frame_and_source_boundary_method() {
    let input = load_blocking(&root()).unwrap();
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
    let input = load_blocking(&root()).unwrap();
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
    let input = load_blocking(&root()).unwrap();
    let mut parameters = controls();
    parameters.target_frame = TargetFrame {
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
    std::fs::write(&parameters, serde_json::to_vec(&controls()).unwrap()).unwrap();
    let registered = temp.path().join("registered");
    boundary_blocking(&root(), &path, Some(&parameters), &registered).unwrap();
    assert!(registered.join("boundary.json").is_file());
    assert!(registered.join("boundary-trace.json").is_file());
}
