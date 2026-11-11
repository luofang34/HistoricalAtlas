use super::*;

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
