use super::*;
use crate::registration::{SYNTHETIC_REFERENCE, with_synthetic_reference};

#[cfg(test)]
fn job() -> RegistrationJob {
    let controls = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| ReferencedControl {
            basis: Vec::new(),
            landmark: format!("control-{index}"),
            reference: "test coordinate frame".into(),
            reference_sha256: Some(SYNTHETIC_REFERENCE.into()),
            uncertainty: None,
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
        scan_page: Some(40),
        source_frame: RasterFrame {
            width: 100,
            height: 100,
        },
        target_frame: TargetFrame {
            datum: None,
            crs: "EPSG:32648".into(),
            units: "metres".into(),
        },
        method: RegistrationMethod::Affine,
        controls,
        check_points: vec![ReferencedControl {
            basis: Vec::new(),
            landmark: "holdout".into(),
            reference: "test coordinate frame".into(),
            reference_sha256: Some(SYNTHETIC_REFERENCE.into()),
            uncertainty: None,
            point: ControlPoint {
                source: PlanarPoint { x: 50.0, y: 50.0 },
                target: PlanarPoint {
                    x: 400_100.0,
                    y: 2_999_900.0,
                },
            },
        }],
        vertices: vec![PlanarPoint { x: 25.0, y: 25.0 }],
        terrain: None,
    }
}

#[test]
fn registration_command_path_writes_quality_and_refuses_overwrite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let directory = tempfile::tempdir().unwrap();
    let request = directory.path().join("controls.json");
    let output = directory.path().join("registered.json");
    // The command reloads the ledger from disk, so its controls cite a checked fact there.
    let fact = load_blocking(&root)
        .unwrap()
        .data
        .records
        .iter()
        .find(|record| record.checked)
        .unwrap()
        .id;
    let mut job = job();
    for control in job.controls.iter_mut().chain(&mut job.check_points) {
        control.reference_sha256 = None;
        control.basis.push(fact);
    }
    std::fs::write(&request, serde_json::to_vec(&job).unwrap()).unwrap();
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
    let mut input = with_synthetic_reference(load_blocking(&root).unwrap());
    input.derivations.clear();
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
    let mut input = with_synthetic_reference(load_blocking(&root).unwrap());
    input.derivations.clear();
    let mut request = job();
    request.check_points = vec![request.controls[0].clone()];
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("fitting control")
    );
    let mut request = job();
    request.scan_page = Some(500);
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("has no fingerprint")
    );
    let mut request = job();
    request.source_id = "nasa-nssdca-apollo11".into();
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("has no fingerprint")
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

#[cfg(test)]
fn capture_input() -> LedgerInput {
    let catalog = serde_json::from_value(serde_json::json!({"sources":[{
        "id":"frame","title":"Test reconnaissance frame","capture":{"medium":"aerial_photograph",
        "frame":"17","sortie":"A","sha256":"a".repeat(64),"archive":{"repository":"Test archive","accession":"AIR/17"}}
    }]})).unwrap();
    with_synthetic_reference(LedgerInput {
        catalog,
        data: Default::default(),
        aliases: Default::default(),
        published: Default::default(),
        derivations: std::collections::BTreeMap::new(),
        terrains: std::collections::BTreeMap::new(),
        traces: Vec::new(),
        place_maps: std::collections::BTreeMap::new(),
        plate_models: std::collections::BTreeMap::new(),
    })
}

#[test]
fn unpaginated_aerial_capture_uses_its_fingerprint_and_rejects_unknown_control_facts() {
    let input = capture_input();
    let mut request = job();
    request.source_id = "frame".into();
    request.scan_page = None;
    let result = register(&input, request.clone()).unwrap();
    assert_eq!(result.scan_sha256, "a".repeat(64));
    assert_eq!(
        result.transform(request.vertices[0]).unwrap(),
        result.vertices[0]
    );
    request.controls[0]
        .basis
        .push(atlas_core::identity::FactId::from_key("missing/fact").unwrap());
    assert!(
        register(&input, request)
            .unwrap_err()
            .to_string()
            .contains("unchecked fact")
    );
}

#[test]
fn controls_without_checked_facts_or_a_fingerprinted_reference_are_rejected() {
    let mut request = job();
    request.source_id = "frame".into();
    request.scan_page = None;
    request.controls[1].reference_sha256 = None;
    let error = register(&capture_input(), request.clone()).unwrap_err();
    assert!(
        error.to_string().contains("fingerprinted reference"),
        "{error}"
    );
    // The capture itself is catalogued but carries no rights statement.
    request.controls[1].reference_sha256 = Some("a".repeat(64));
    let error = register(&capture_input(), request.clone()).unwrap_err();
    assert!(error.to_string().contains("rights statement"), "{error}");
    request.controls[1].reference_sha256 = Some("f".repeat(64));
    assert!(register(&capture_input(), request.clone()).is_err());
    request.controls[1].reference_sha256 = Some("not-a-digest".into());
    assert!(register(&capture_input(), request.clone()).is_err());
    request.controls[1].reference_sha256 = Some(SYNTHETIC_REFERENCE.into());
    request.controls[1].uncertainty = Some(-1.0);
    assert!(register(&capture_input(), request).is_err());
}

#[test]
fn projective_capture_registration_reports_holdouts_and_refuses_extrapolation() {
    let ground = |x: f64, y: f64| {
        let w = 1.0 + 0.004 * y;
        PlanarPoint {
            x: 600_000.0 + 3.0 * x / w,
            y: 8_957_000.0 + (500.0 - 4.0 * y) / w,
        }
    };
    let mut request = job();
    request.source_id = "frame".into();
    request.scan_page = None;
    request.method = RegistrationMethod::Projective;
    for control in &mut request.controls {
        let source = control.point.source;
        control.point.target = ground(source.x, source.y);
    }
    request.check_points[0].point.target = ground(50.0, 50.0);
    request.vertices = vec![PlanarPoint { x: 25.0, y: 75.0 }];
    let result = register(&capture_input(), request.clone()).unwrap();
    assert!(result.check_quality.as_ref().unwrap().maximum < 1e-6);
    let expected = ground(25.0, 75.0);
    assert!((result.vertices[0].x - expected.x).hypot(result.vertices[0].y - expected.y) < 1e-6);
    request.vertices = vec![
        PlanarPoint { x: 100.0, y: 0.0 },
        PlanarPoint { x: 0.0, y: 0.5 },
    ];
    assert!(register(&capture_input(), request.clone()).is_ok());
    request.source_frame.width = 200;
    request.vertices = vec![PlanarPoint { x: 150.0, y: 50.0 }];
    let error = register(&capture_input(), request).unwrap_err();
    assert!(error.to_string().contains("convex hull"), "{error}");
}
