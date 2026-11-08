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

#[test]
fn world_view_and_polar_drags_stay_inside_the_flat_map() {
    let mut camera = Camera::for_body(BodyId::Earth);
    camera.zoom_at(-20.0, [0.0, 0.0]);
    camera.fit_height(900.0);
    let world = 512.0 * 2.0_f64.powf(camera.zoom);
    assert!((world - 900.0).abs() < 1e-10);
    assert!(camera.center.latitude.abs() < 1e-10);
    camera.zoom_at(3.0, [0.0, 0.0]);
    for drag in [-1e6, 1e6] {
        camera.pan(0.0, drag);
        camera.fit_height(900.0);
        let half = 900.0 / (512.0 * 2.0_f64.powf(camera.zoom)) / 2.0;
        let y = project(camera.center)[1];
        assert!(y - half >= -1e-12 && y + half <= 1.0 + 1e-12);
    }
}

#[test]
fn geographic_extent_fits_both_wide_and_narrow_viewports() {
    let bounds = [
        LonLat {
            longitude: 104.85,
            latitude: 27.05,
        },
        LonLat {
            longitude: 105.95,
            latitude: 27.78,
        },
    ];
    for viewport in [[800.0, 600.0], [320.0, 700.0], [900.0, 240.0]] {
        let mut camera = Camera::for_body(BodyId::Earth);
        camera.fit_bounds(bounds, viewport);
        let center = project(camera.center);
        let scale = 512.0 * 2.0_f64.powf(camera.zoom);
        for corner in bounds {
            let point = project(corner);
            for axis in 0..2 {
                assert!((point[axis] - center[axis]).abs() * scale < viewport[axis] * 0.43);
            }
        }
    }
}
