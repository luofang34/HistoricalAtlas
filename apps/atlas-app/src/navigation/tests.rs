use super::*;

#[test]
fn pan_wraps_and_zoom_preserves_the_pointer_anchor() {
    let mut camera = Camera::for_body(BodyId::Earth);
    let original = project(camera.center);
    let anchor = [120.0, -40.0];
    let scale = 512.0 * 2.0_f64.powf(camera.zoom);
    let target = [
        original[0] + anchor[0] / scale,
        original[1] + anchor[1] / scale,
    ];
    camera.zoom_at(2.0, anchor);
    let result = project(camera.center);
    let scale = 512.0 * 2.0_f64.powf(camera.zoom);
    assert!((result[0] + anchor[0] / scale - target[0]).abs() < 1e-12);
    assert!((result[1] + anchor[1] / scale - target[1]).abs() < 1e-12);
    camera.pan(1e12, -1e12);
    assert!((-180.0..180.0).contains(&camera.center.longitude));
    assert!(camera.center.latitude.abs() <= 85.051_128_779_806_6);
}
