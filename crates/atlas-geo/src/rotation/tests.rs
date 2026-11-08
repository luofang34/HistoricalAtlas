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
