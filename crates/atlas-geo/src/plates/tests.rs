use super::*;

#[test]
fn plates4_parser_retains_source_lines_and_skips_disabled_rotations() {
    let model = RotationModel::from_rot(
        "# header\n999 disabled\n602 10 90 360 10 0 ! South China\n602 0 90 0 0 0 ! present\n",
    )
    .unwrap();
    let sequence = model.sequences().next().unwrap();
    assert_eq!(sequence.samples[0].source_line, 4);
    assert_eq!(sequence.samples[1].rotation.pole.longitude, 0.0);
    assert_eq!(sequence.samples[1].description, "South China");
    let result = model.rotation(PlateId(602), 5.0, PlateId(0)).unwrap();
    assert!((result.rotation.angle_degrees - 5.0).abs() < 1e-10);
    assert_eq!(result.circuit[0].younger_line, 4);
    assert_eq!(result.circuit[0].older_line, 3);
}

#[test]
fn circuit_composes_noncommuting_rotations_in_geographic_order() {
    let model = RotationModel::from_rot("1 0 90 0 90 2\n2 0 0 0 90 0").unwrap();
    let point = LonLat {
        longitude: 0.0,
        latitude: 0.0,
    };
    let result = model
        .reconstruct(point, PlateId(1), 0.0, PlateId(0))
        .unwrap();
    assert!((result.latitude - 90.0).abs() < 1e-10);
    let anchored = model
        .reconstruct(point, PlateId(1), 0.0, PlateId(2))
        .unwrap();
    assert!((anchored.longitude - 90.0).abs() < 1e-10);
    assert!(anchored.latitude.abs() < 1e-10);
    let same = model.rotation(PlateId(1), 0.0, PlateId(1)).unwrap();
    assert!(
        same.rotation
            .separation_degrees(EulerRotation::IDENTITY)
            .unwrap()
            < 1e-10
    );
    assert_eq!(same.anchor_circuit.len(), 2);
}

#[test]
fn crossover_selects_older_history_and_preserves_continuity() {
    let model = RotationModel::from_rot(
        "1 0 90 0 0 0\n1 10 90 0 20 0\n1 10 90 0 10 2\n1 20 90 0 30 2\n2 0 90 0 10 0\n2 20 90 0 10 0",
    ).unwrap();
    let at = model.rotation(PlateId(1), 10.0, PlateId(0)).unwrap();
    let before = model.rotation(PlateId(1), 9.999999, PlateId(0)).unwrap();
    let after = model.rotation(PlateId(1), 10.000001, PlateId(0)).unwrap();
    assert_eq!(at.circuit[0].fixed_plate, PlateId(2));
    assert_eq!(before.circuit[0].fixed_plate, PlateId(0));
    assert!(at.rotation.separation_degrees(before.rotation).unwrap() < 3e-6);
    assert!(at.rotation.separation_degrees(after.rotation).unwrap() < 3e-6);
}

#[test]
fn equivalent_duplicate_ages_are_valid_but_conflicting_samples_are_rejected() {
    let model =
        RotationModel::from_rot("1 0 90 0 0 0\n1 10 90 0 90 0\n1 10 -90 0 -90 0\n1 20 90 0 180 0")
            .unwrap();
    assert!(
        (model
            .rotation(PlateId(1), 15.0, PlateId(0))
            .unwrap()
            .rotation
            .angle_degrees
            - 135.0)
            .abs()
            < 1e-10
    );
    assert!(matches!(
        RotationModel::from_rot("1 10 90 0 0 0\n1 10 90 0 1 0"),
        Err(PlateModelError::Sequence { .. })
    ));
}

#[test]
fn missing_edges_and_cycles_fail_instead_of_reconstructing_stationary_plates() {
    let model = RotationModel::from_rot("1 0 90 0 0 2\n1 10 90 0 10 2").unwrap();
    assert!(matches!(
        model.rotation(PlateId(1), 5.0, PlateId(0)),
        Err(PlateModelError::Missing {
            plate: PlateId(2),
            ..
        })
    ));
    for age in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            model.rotation(PlateId(0), age, PlateId(0)),
            Err(PlateModelError::Age(_))
        ));
    }
    for age in [0.0, 11.0] {
        assert!(model.rotation(PlateId(3), age, PlateId(3)).is_err());
    }
    assert!(model.rotation(PlateId(1), 11.0, PlateId(0)).is_err());
    let cycle = RotationModel::from_rot("1 0 90 0 0 2\n2 0 90 0 0 1").unwrap();
    assert!(matches!(
        cycle.rotation(PlateId(1), 0.0, PlateId(0)),
        Err(PlateModelError::Cycle { .. })
    ));
}

#[test]
fn invalid_input_and_overlapping_histories_fail_with_context() {
    for text in [
        "1 0 90",
        "plate 0 90 0 0 0",
        "1 age 90 0 0 0",
        "1 -1 90 0 0 0",
        "1 NaN 90 0 0 0",
        "1 0 91 0 0 0",
        "1 0 90 inf 0 0",
        "1 0 90 0 NaN 0",
        "0 0 90 0 0 1",
        "1 0 90 0 0 1",
    ] {
        assert!(RotationModel::from_rot(text).is_err(), "{text}");
    }
    let overlap = "1 0 90 0 0 0\n1 20 90 0 0 0\n1 10 90 0 0 2\n1 30 90 0 0 2";
    assert!(matches!(
        RotationModel::from_rot(overlap),
        Err(PlateModelError::Sequence {
            plate: PlateId(1),
            ..
        })
    ));
}
