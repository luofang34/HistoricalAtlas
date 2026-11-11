use super::*;

#[cfg(test)]
fn point(longitude: f64, latitude: f64, height_m: f64) -> GeodeticPoint {
    GeodeticPoint {
        position: LonLat {
            longitude,
            latitude,
        },
        height_m,
    }
}

#[test]
fn local_frame_round_trips_and_measures_metric_offsets() {
    let frame = LocalFrame::new(point(160.0547, -9.4281, 20.0));
    let origin = frame.to_local(point(160.0547, -9.4281, 20.0));
    assert!(origin.norm() < 1e-6);
    // One arc-minute of latitude near 9° S is about 1843 m on WGS84.
    let north = frame.to_local(point(160.0547, -9.4281 + 1.0 / 60.0, 20.0));
    assert!((north.y - 1843.0).abs() < 2.0, "{north:?}");
    assert!(north.x.abs() < 1e-3);
    let up = frame.to_local(point(160.0547, -9.4281, 1020.0));
    assert!((up.z - 1000.0).abs() < 1e-6);
    let back = frame.to_geodetic(frame.to_local(point(160.1, -9.45, 350.0)));
    assert!((back.position.longitude - 160.1).abs() < 1e-9);
    assert!((back.position.latitude + 9.45).abs() < 1e-9);
    assert!((back.height_m - 350.0).abs() < 1e-4);
}

#[test]
fn lunar_frame_uses_the_moon_sphere() {
    let moon = crate::body::BodyId::Moon.ellipsoid();
    let frame = LocalFrame::with_ellipsoid(moon, point(23.473, 0.674, 0.0));
    // One arc-minute on the 1737.4 km lunar sphere is about 505.4 m.
    let north = frame.to_local(point(23.473, 0.674 + 1.0 / 60.0, 0.0));
    assert!((north.y - 505.4).abs() < 0.5, "{north:?}");
    let back = frame.to_geodetic(frame.to_local(point(23.5, 0.7, -1200.0)));
    assert!((back.height_m + 1200.0).abs() < 1e-4);
    assert!((back.position.latitude - 0.7).abs() < 1e-9);
}

#[test]
fn polar_coordinates_round_trip_without_height_singularity() {
    for (latitude, height_m) in [(90.0, 0.0), (90.0, 2835.0), (-90.0, 0.0), (-89.75, 1500.0)] {
        let origin = point(45.0, latitude, height_m);
        let frame = LocalFrame::new(origin);
        let recovered = frame.to_geodetic(Vector3::zeros());
        assert!((recovered.position.latitude - latitude).abs() < 1e-9);
        assert!(
            (recovered.height_m - height_m).abs() < 1e-3,
            "expected {height_m}, got {}",
            recovered.height_m
        );
    }
}
