use super::*;

#[cfg(test)]
fn transformed(source: PlanarPoint) -> PlanarPoint {
    PlanarPoint {
        x: 400_000.0 + 2.0 * source.x - 0.3 * source.y,
        y: 3_000_000.0 + 0.2 * source.x - 1.5 * source.y,
    }
}

#[cfg(test)]
fn controls() -> Vec<ControlPoint> {
    [
        (0.0, 0.0),
        (1200.0, 0.0),
        (1200.0, 1600.0),
        (0.0, 1600.0),
        (650.0, 700.0),
    ]
    .into_iter()
    .map(|(x, y)| {
        let source = PlanarPoint { x, y };
        ControlPoint {
            source,
            target: transformed(source),
        }
    })
    .collect()
}

#[test]
fn affine_registration_recovers_projected_coordinates_and_holdout_error() {
    let model = AffineRegistration::fit(&controls()).unwrap();
    let point = PlanarPoint { x: 312.0, y: 984.0 };
    let result = model.transform(point).unwrap();
    assert!(result.distance(transformed(point)) < 1e-8);
    assert!(model.evaluate(&controls()).unwrap().maximum < 1e-8);
    let mut target = transformed(point);
    target.x += 3.0;
    target.y += 4.0;
    let quality = model
        .evaluate(&[ControlPoint {
            source: point,
            target,
        }])
        .unwrap();
    assert!((quality.rms - 5.0).abs() < 1e-8);
}

#[test]
fn thin_plate_interpolates_a_warp_and_preserves_an_affine_mapping() {
    let points = controls();
    let affine = ThinPlateSpline::fit(&points, 0.0).unwrap();
    let point = PlanarPoint { x: 100.0, y: 200.0 };
    assert!(
        affine
            .transform(point)
            .unwrap()
            .distance(transformed(point))
            < 1e-8
    );
    let mut warped = points;
    warped[4].target.x += 40.0;
    warped[4].target.y += 20.0;
    let model = ThinPlateSpline::fit(&warped, 0.0).unwrap();
    assert!(model.evaluate(&warped).unwrap().maximum < 1e-7);
    let interior = PlanarPoint { x: 500.0, y: 600.0 };
    assert!(
        model
            .transform(interior)
            .unwrap()
            .distance(transformed(interior))
            > 1.0
    );
    let smooth = ThinPlateSpline::fit(&warped, 1.0).unwrap();
    assert!(smooth.evaluate(&warped).unwrap().rms > 0.0);
}

#[test]
fn degenerate_and_nonfinite_controls_are_rejected() {
    let collinear: Vec<_> = (0..4)
        .map(|n| {
            let source = PlanarPoint {
                x: f64::from(n),
                y: f64::from(n),
            };
            ControlPoint {
                source,
                target: transformed(source),
            }
        })
        .collect();
    assert!(AffineRegistration::fit(&collinear).is_err());
    assert!(ThinPlateSpline::fit(&collinear, 0.0).is_err());
    assert!(ThinPlateSpline::fit(&controls(), -1.0).is_err());
    let mut points = controls();
    points[0].source.x = f64::NAN;
    assert!(AffineRegistration::fit(&points).is_err());
    let mut points = controls();
    points[1] = points[0];
    assert!(ThinPlateSpline::fit(&points, 1.0).is_err());
    assert!(AffineRegistration::fit(&[]).is_err());
}

/// Ground metres seen by a camera tilted 60 degrees from vertical over flat terrain.
#[cfg(test)]
fn oblique(source: PlanarPoint) -> Option<PlanarPoint> {
    let w = 1.0 + 0.0009 * source.y - 0.00004 * source.x;
    (w > 0.0).then(|| PlanarPoint {
        x: 712_400.0 + (1.8 * source.x + 0.4 * source.y) / w,
        y: 8_957_300.0 + (0.3 * source.x - 2.6 * source.y + 900.0) / w,
    })
}

#[cfg(test)]
fn oblique_controls(points: &[(f64, f64)]) -> Vec<ControlPoint> {
    points
        .iter()
        .map(|&(x, y)| {
            let source = PlanarPoint { x, y };
            ControlPoint {
                source,
                target: oblique(source).unwrap(),
            }
        })
        .collect()
}

#[test]
fn projective_registration_recovers_oblique_ground_plane_and_measures_holdouts() {
    let controls = oblique_controls(&[
        (100.0, 300.0),
        (1500.0, 280.0),
        (1650.0, 1100.0),
        (60.0, 1150.0),
        (820.0, 640.0),
    ]);
    let model = ProjectiveRegistration::fit(&controls).unwrap();
    assert!(model.evaluate(&controls).unwrap().maximum < 1e-6);
    let holdout = oblique_controls(&[(400.0, 900.0), (1300.0, 500.0)]);
    assert!(model.evaluate(&holdout).unwrap().maximum < 1e-6);
    // Affine cannot represent perspective foreshortening.
    let affine = AffineRegistration::fit(&controls).unwrap();
    assert!(affine.evaluate(&holdout).unwrap().maximum > 10.0);
}

#[test]
fn projective_registration_rejects_horizon_and_degenerate_controls() {
    let controls = oblique_controls(&[
        (100.0, 300.0),
        (1500.0, 280.0),
        (1650.0, 1100.0),
        (60.0, 1150.0),
    ]);
    let model = ProjectiveRegistration::fit(&controls).unwrap();
    // w = 0 at y ≈ -1111 for x = 0; points above that line are sky.
    let sky = PlanarPoint { x: 0.0, y: -1500.0 };
    assert!(matches!(
        model.transform(sky),
        Err(RegistrationError::BeyondHorizon(_))
    ));
    assert!(matches!(
        ProjectiveRegistration::fit(&controls[..3]),
        Err(RegistrationError::TooFewPoints { minimum: 4, .. })
    ));
    let collinear = oblique_controls(&[(0.0, 0.0), (100.0, 100.0), (200.0, 200.0), (300.0, 300.0)]);
    assert!(ProjectiveRegistration::fit(&collinear).is_err());
}

#[test]
fn control_support_excludes_points_outside_the_fitting_hull() {
    let support = ControlSupport::new(&controls()).unwrap();
    assert_eq!(support.vertices().len(), 4);
    assert!(support.contains(PlanarPoint { x: 650.0, y: 700.0 }));
    assert!(support.contains(PlanarPoint { x: 0.0, y: 800.0 }));
    assert!(matches!(
        support.require(PlanarPoint { x: -1.0, y: 800.0 }),
        Err(RegistrationError::OutsideSupport(_))
    ));
}
