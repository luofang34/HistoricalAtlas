use super::*;

#[test]
fn finite_rotation_and_inverse_keep_anchor_intact() {
    let anchor = LonLat {
        longitude: 0.0,
        latitude: 0.0,
    };
    let rotation = EulerRotation {
        pole: LonLat {
            longitude: 0.0,
            latitude: 90.0,
        },
        angle_degrees: 90.0,
    };
    let turned = rotation.apply(anchor).unwrap();
    assert!((turned.longitude - 90.0).abs() < 1e-10);
    assert!(turned.latitude.abs() < 1e-10);
    let inverse = EulerRotation {
        angle_degrees: -90.0,
        ..rotation
    };
    let returned = inverse.apply(turned).unwrap();
    assert!(returned.longitude.abs() < 1e-10);
    assert_eq!(anchor.longitude, 0.0);
}

#[test]
fn quaternion_interpolation_handles_identity_and_shortest_arc() {
    let north = EulerRotation {
        angle_degrees: 170.0,
        ..EulerRotation::IDENTITY
    };
    let south = EulerRotation {
        angle_degrees: -170.0,
        ..EulerRotation::IDENTITY
    };
    let middle = north.interpolate(south, 0.5).unwrap();
    assert!((middle.angle_degrees.abs() - 180.0).abs() < 1e-10);
    assert!(
        north
            .interpolate(north, 0.5)
            .unwrap()
            .separation_degrees(north)
            .unwrap()
            < 1e-10
    );
    assert_eq!(
        EulerRotation::IDENTITY
            .interpolate(EulerRotation::IDENTITY, 0.5)
            .unwrap(),
        EulerRotation::IDENTITY
    );
    let full = EulerRotation {
        angle_degrees: 360.0,
        ..EulerRotation::IDENTITY
    };
    assert!(
        full.interpolate(EulerRotation::IDENTITY, 0.5)
            .unwrap()
            .separation_degrees(EulerRotation::IDENTITY)
            .unwrap()
            < 1e-10
    );
    for fraction in [-0.1, 1.1, f64::NAN] {
        assert!(north.interpolate(south, fraction).is_err());
    }
}

#[test]
fn interpolation_between_different_axes_has_an_analytical_midpoint() {
    let x = EulerRotation {
        pole: LonLat {
            longitude: 0.0,
            latitude: 0.0,
        },
        angle_degrees: 180.0,
    };
    let y = EulerRotation {
        pole: LonLat {
            longitude: 90.0,
            latitude: 0.0,
        },
        angle_degrees: 180.0,
    };
    let midpoint = x.interpolate(y, 0.5).unwrap();
    let point = midpoint
        .apply(LonLat {
            longitude: 0.0,
            latitude: 0.0,
        })
        .unwrap();
    assert!((point.longitude - 90.0).abs() < 1e-10);
    assert!(point.latitude.abs() < 1e-10);
    assert!(
        midpoint
            .then(midpoint.inverse().unwrap())
            .unwrap()
            .separation_degrees(EulerRotation::IDENTITY)
            .unwrap()
            < 1e-10
    );
}
