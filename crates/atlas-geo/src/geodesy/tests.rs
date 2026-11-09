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
