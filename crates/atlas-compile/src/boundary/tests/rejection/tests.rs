use super::*;

#[test]
fn ordinary_ledger_loading_checks_curated_trace_documents() {
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let temp = tempfile::tempdir().unwrap();
    let mut catalog = input.catalog;
    catalog
        .sources
        .retain(|source| source.id == "zhijin-gazetteer-scan-b");
    std::fs::write(
        temp.path().join("sources.toml"),
        toml::to_string(&catalog).unwrap(),
    )
    .unwrap();
    std::fs::copy(
        root().join("guizhou-1960s/zhijin/ertang.dataset.json"),
        temp.path().join("ertang.dataset.json"),
    )
    .unwrap();
    std::fs::write(temp.path().join("aliases.json"), "{}").unwrap();
    std::fs::write(temp.path().join("published.json"), "[]").unwrap();
    let path = temp.path().join("ertang.trace.json");
    let mut request = trace();
    std::fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
    assert!(load_blocking(temp.path()).is_ok());
    request.units[0].polygons[0].exterior[1] = 999;
    std::fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
    let error = load_blocking(temp.path()).unwrap_err();
    assert!(matches!(error, crate::input::InputError::Trace { .. }));
    assert!(error.to_string().contains("missing boundary vertex 999"));
}

#[test]
fn trace_validation_rejects_wrong_editions_pages_and_unchecked_records() {
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let mut request = trace();
    request.scan_page = 41;
    assert!(
        compile_trace(&input, request)
            .unwrap_err()
            .to_string()
            .contains("traced sheet")
    );
    let mut request = trace();
    request.source_id = "zhijin-gazetteer-scan-a".into();
    assert!(compile_trace(&input, request).is_err());
    let request = trace();
    input
        .data
        .records
        .iter_mut()
        .find(|record| record.id == request.units[0].record_id)
        .unwrap()
        .checked = false;
    assert!(compile_trace(&input, request).is_err());
}

#[test]
fn geometry_and_method_claims_require_matching_evidence() {
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let mut request = trace();
    request.method_basis.clear();
    assert!(compile_trace(&input, request).is_err());
    let mut request = trace();
    request.units[0].polygons[0].exterior.pop();
    assert!(compile_trace(&input, request).is_err());
    let mut request = trace();
    request.vertices[0].x = 1047.;
    assert!(compile_trace(&input, request).is_err());
    let mut request = trace();
    request.units.push(request.units[0].clone());
    assert!(compile_trace(&input, request).is_err());
}

#[test]
fn nonlinear_registration_rejects_a_fold_in_valid_source_linework() {
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let mut request = trace();
    request.vertices = vec![
        PlanarPoint { x: 0., y: 0. },
        PlanarPoint { x: 1000., y: 0. },
        PlanarPoint { x: 1000., y: 1000. },
        PlanarPoint { x: 0., y: 1000. },
    ];
    request.units[0].polygons[0].exterior = vec![0, 1, 2, 3, 0];
    let mut parameters = controls();
    parameters.method = RegistrationMethod::ThinPlate { smoothing: 0. };
    parameters.check_points.clear();
    parameters.controls = request
        .vertices
        .iter()
        .enumerate()
        .map(|(index, point)| ReferencedControl {
            basis: Vec::new(),
            landmark: format!("corner-{index}"),
            reference: "synthetic folded map test".into(),
            point: ControlPoint {
                source: *point,
                target: request.vertices[[0, 2, 1, 3][index]],
            },
        })
        .collect();
    let error = compile_boundary(&input, request, parameters).unwrap_err();
    assert!(matches!(
        error,
        BoundaryError::Topology {
            frame: "registered coordinates",
            ..
        }
    ));
}

#[test]
fn crs84_rejects_non_geographic_units_and_other_bodies() {
    let mut input = load_blocking(&root()).unwrap();
    input.derivations.clear();
    let mut parameters = controls();
    parameters.target_frame.units = "metres".into();
    assert!(compile_boundary(&input, trace(), parameters).is_err());
    let request = trace();
    let entity_id = evidence::record(&input, request.units[0].record_id)
        .unwrap()
        .entity_id;
    input
        .data
        .entities
        .iter_mut()
        .find(|entity| entity.id == entity_id)
        .unwrap()
        .body = atlas_geo::body::BodyId::Moon;
    assert!(compile_boundary(&input, request, controls()).is_err());
}
