use super::*;

#[cfg(test)]
fn job() -> RegistrationJob {
    let controls = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| ReferencedControl {
            landmark: format!("control-{index}"),
            reference: "test coordinate frame".into(),
            point: ControlPoint {
                source: PlanarPoint { x, y },
                target: PlanarPoint {
                    x: 2.0 * x + 400_000.0,
                    y: 3_000_000.0 - 2.0 * y,
                },
            },
        })
        .collect();
    RegistrationJob {
        source_id: "zhijin-gazetteer-scan-b".into(),
        scan_page: 40,
        source_frame: RasterFrame {
            width: 100,
            height: 100,
        },
        target_frame: TargetFrame {
            crs: "EPSG:32648".into(),
            units: "metres".into(),
        },
        method: RegistrationMethod::Affine,
        controls,
        check_points: vec![ReferencedControl {
            landmark: "holdout".into(),
            reference: "test coordinate frame".into(),
            point: ControlPoint {
                source: PlanarPoint { x: 50.0, y: 50.0 },
                target: PlanarPoint {
                    x: 400_100.0,
                    y: 2_999_900.0,
                },
            },
        }],
        vertices: vec![PlanarPoint { x: 25.0, y: 25.0 }],
    }
}

#[test]
fn registration_command_path_writes_quality_and_refuses_overwrite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let directory = tempfile::tempdir().unwrap();
    let request = directory.path().join("controls.json");
    let output = directory.path().join("registered.json");
    std::fs::write(&request, serde_json::to_vec(&job()).unwrap()).unwrap();
    let result = register_blocking(&root, &request, &output).unwrap();
    assert!(result.control_quality.rms < 1e-8);
    assert!(result.check_quality.unwrap().maximum < 1e-8);
    assert_eq!(
        result.vertices[0],
        PlanarPoint {
            x: 400_050.0,
            y: 2_999_950.0
        }
    );
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    assert_eq!(written["job"]["target_frame"]["units"], "metres");
    assert_eq!(written["job"]["source_frame"]["width"], 100);
    assert_eq!(
        written["scan_sha256"],
        "11dec198a03f98e48799cbf142c77845e5ee004a9bfd395f1c5f5f0e2ad9c66e"
    );
    assert!(register_blocking(&root, &request, &output).is_err());
}

#[test]
fn raster_frame_rejects_outside_vertices_and_empty_dimensions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let mut request = job();
    request.vertices[0].x = 101.0;
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("outside")
    );
    let mut request = job();
    request.source_frame.height = 0;
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("positive")
    );
}

#[test]
fn check_points_and_source_page_must_be_independent_and_identified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let mut request = job();
    request.check_points = vec![request.controls[0].clone()];
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("fitting control")
    );
    let mut request = job();
    request.scan_page = 500;
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("out of range")
    );
    let mut request = job();
    request.controls[0].reference.clear();
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("target reference")
    );
}
