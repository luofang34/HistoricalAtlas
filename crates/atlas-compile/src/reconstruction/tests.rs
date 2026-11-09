use super::*;
use atlas_geo::coordinates::Datum;

#[cfg(test)]
fn real_input() -> (LedgerInput, ReconstructionJob) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    input.derivations.clear();
    let job =
        read_json_blocking(&root.join("paleogeography/panxian-240ma.reconstruction.json")).unwrap();
    (input, job)
}

#[test]
fn panxian_reconstruction_keeps_original_evidence_and_resolves_the_africa_circuit() {
    let (input, job) = real_input();
    let result = reconstruct(&input, job.clone()).unwrap();
    assert_eq!(result.age_ma, 240.0);
    let anchor = &result.anchors[0];
    assert_eq!(anchor.observation.datum, Datum::Unspecified);
    assert_eq!(anchor.observation.longitude_text, "E104 53 57.206");
    assert_eq!(anchor.observation.latitude_text, "N25 31 36.880");
    assert!((anchor.reconstructed.longitude - 100.7314910987376).abs() < 1e-8);
    assert!((anchor.reconstructed.latitude - 18.574855873688293).abs() < 1e-8);
    let circuit: Vec<_> = anchor
        .motion
        .circuit
        .iter()
        .map(|step| {
            (
                step.moving_plate.0,
                step.fixed_plate.0,
                step.younger_line,
                step.older_line,
            )
        })
        .collect();
    assert_eq!(
        circuit,
        vec![
            (602, 701, 1408, 1409),
            (701, 70, 1883, 1883),
            (70, 0, 13, 13)
        ]
    );
    let artifacts = compile_reconstruction(&input, job).unwrap();
    assert!(artifacts.contains_key(Path::new("plates/merdith2021-south-china.json")));
    assert!(artifacts.contains_key(Path::new("reconstruction.json")));
    assert!(
        artifacts
            .values()
            .all(|bytes| bytes.len() < crate::export::MAX_FILE_BYTES)
    );
}

#[test]
fn real_model_crossover_and_present_reconstruction_are_consistent() {
    let (input, mut job) = real_input();
    for (age, expected_lon, expected_lat) in [
        (0, 104.8992238888889, 25.52691111111111),
        (150, 129.75640284760223, 30.19401461513475),
        (200, 146.44728307311703, 51.09028003095047),
        (245, 99.32629509099314, 11.987483969983764),
        (250, 98.29104697576213, 5.309866052985359),
    ] {
        job.at = EpochTime::years_before(EpochTime::from_ticks(0), age * 1_000_000).unwrap();
        let result = reconstruct(&input, job.clone()).unwrap();
        let point = result.anchors[0].reconstructed;
        assert!((point.longitude - expected_lon).abs() < 1e-8, "{age}");
        assert!((point.latitude - expected_lat).abs() < 1e-8, "{age}");
    }
}

#[test]
fn unchecked_mismatched_and_out_of_range_requests_are_rejected() {
    let (mut input, job) = real_input();
    let mut bad = job.clone();
    bad.at = EpochTime::years_before(EpochTime::from_ticks(0), 251_000_000).unwrap();
    assert!(matches!(
        reconstruct(&input, bad),
        Err(ReconstructionError::Age { .. })
    ));
    let mut bad = job.clone();
    bad.anchor_plate = PlateId(999);
    assert!(matches!(
        reconstruct(&input, bad),
        Err(ReconstructionError::Circuit(_))
    ));
    let mut bad = job.clone();
    bad.anchors[0].plate_estimate = input
        .data
        .estimates
        .iter()
        .find(|estimate| matches!(estimate.value, EstimateValue::Origin { .. }))
        .unwrap()
        .id;
    assert!(matches!(
        reconstruct(&input, bad),
        Err(ReconstructionError::Evidence(_))
    ));
    input
        .data
        .records
        .iter_mut()
        .find(|record| record.id == job.anchors[0].coordinate_record)
        .unwrap()
        .checked = false;
    assert!(reconstruct(&input, job).is_err());
}

#[test]
fn changed_model_metadata_and_missing_reference_edges_fail_publication() {
    let (mut input, job) = real_input();
    input
        .plate_models
        .get_mut(&job.model_id)
        .unwrap()
        .source_sha256 = "invalid".into();
    assert!(crate::export::compile_with_derived(&input, &[]).is_err());
    let (mut input, job) = real_input();
    input
        .plate_models
        .get_mut(&job.model_id)
        .unwrap()
        .sequences
        .retain(|sequence| sequence.moving_plate != PlateId(410));
    assert!(crate::export::compile_with_derived(&input, &[]).is_err());
    let (mut input, job) = real_input();
    let estimate = input
        .data
        .estimates
        .iter_mut()
        .find(|estimate| estimate.id == job.anchors[0].plate_estimate)
        .unwrap();
    estimate.basis = vec![job.anchors[0].coordinate_record];
    assert!(crate::export::compile_with_derived(&input, &[]).is_err());
}

#[test]
fn reconstruction_command_writes_evidence_and_refuses_existing_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let request = root.join("paleogeography/panxian-240ma.reconstruction.json");
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("atlas");
    let count = reconstruct_blocking(&root, &request, &output).unwrap();
    assert!(count > 5);
    let result: serde_json::Value =
        read_json_blocking(&output.join("reconstruction.json")).unwrap();
    assert_eq!(result["job"]["at"], "-7573824000000000000");
    assert_eq!(result["anchors"][0]["observation"]["datum"], "unspecified");
    assert!(output.join("sources.json").is_file());
    assert!(reconstruct_blocking(&root, &request, &output).is_err());
    let (_, mut job) = real_input();
    job.model_id = "missing-model".into();
    let invalid = directory.path().join("invalid.json");
    std::fs::write(&invalid, serde_json::to_vec(&job).unwrap()).unwrap();
    let rejected = directory.path().join("rejected");
    assert!(reconstruct_blocking(&root, &invalid, &rejected).is_err());
    assert!(!rejected.exists());
}

#[test]
fn plates4_import_of_cited_samples_resolves_every_crossover() {
    let (input, _) = real_input();
    let document = &input.plate_models["merdith2021-south-china"];
    let mut text = String::new();
    for sequence in &document.sequences {
        for sample in &sequence.samples {
            text.push_str(&format!(
                "{} {} {} {} {} {} ! {}\n",
                sequence.moving_plate.0,
                sample.age_ma,
                sample.rotation.pole.latitude,
                sample.rotation.pole.longitude,
                sample.rotation.angle_degrees,
                sequence.fixed_plate.0,
                sample.description
            ));
        }
    }
    let imported = atlas_geo::plates::RotationModel::from_rot(&text).unwrap();
    let original = document.model(&input.catalog).unwrap();
    for age in [
        0.0, 33.1, 79.1, 120.0, 150.0, 199.999999, 200.0, 200.000001, 240.0, 250.0,
    ] {
        let expected = original.rotation(PlateId(602), age, PlateId(0)).unwrap();
        let actual = imported.rotation(PlateId(602), age, PlateId(0)).unwrap();
        assert!(
            actual
                .rotation
                .separation_degrees(expected.rotation)
                .unwrap()
                < 1e-10
        );
    }
}
