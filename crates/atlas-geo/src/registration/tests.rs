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
