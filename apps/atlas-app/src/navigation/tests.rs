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

#[cfg(test)]
fn globe_view(camera: Camera) -> maplibre::render::view_state::ViewState {
    use maplibre::{
        coords::{LatLon, WorldCoords, Zoom},
        render::view_state::{NavigationMode, ViewState},
        window::PhysicalSize,
    };
    let zoom = Zoom::new(camera.zoom);
    let mut view = ViewState::new(
        PhysicalSize::new(800, 600).unwrap(),
        WorldCoords::from_lat_lon(
            LatLon::new(camera.center.latitude, camera.center.longitude),
            zoom,
        ),
        zoom,
        cgmath::Deg(0.0),
        cgmath::Deg(36.869_897_645_844_02),
    );
    view.set_globe_orbits_center(true);
    view.set_navigation_mode(NavigationMode::FreeGlobe, &PROJECTION)
        .unwrap();
    view
}

#[test]
fn globe_has_a_visible_limb_and_drag_and_zoom_keep_the_ground_anchor() {
    use cgmath::{InnerSpace, Point2};
    use maplibre::render::projection::globe_camera_for_view;
    let mut view = globe_view(Camera::for_body(BodyId::Earth));
    let camera = globe_camera_for_view(&view).unwrap();
    assert!(!camera.is_point_on_map_surface(Point2::new(0.0, 0.0)));
    assert!(camera.is_point_on_map_surface(Point2::new(400.0, 300.0)));
    let from = Point2::new(390.0, 290.0);
    let to = Point2::new(430.0, 315.0);
    let grabbed = camera.screen_point_to_location(from).unwrap();
    assert!(view.drag_free_globe(from, to));
    let moved = globe_camera_for_view(&view)
        .unwrap()
        .screen_point_to_location(to)
        .unwrap();
    assert!((moved.latitude - grabbed.latitude).abs() < 1e-6);
    assert!((moved.longitude - grabbed.longitude).abs() < 1e-6);
    assert!(view.zoom_free_globe(to, 2.0));
    let zoomed = globe_camera_for_view(&view)
        .unwrap()
        .screen_point_to_location(to)
        .unwrap();
    let a = maplibre::projection::globe::lat_lon_to_unit_sphere(grabbed);
    let b = maplibre::projection::globe::lat_lon_to_unit_sphere(zoomed);
    assert!((a - b).magnitude() < 1e-6);
}

#[test]
fn globe_can_rotate_beyond_the_mercator_latitude_limit() {
    use cgmath::Point2;
    use maplibre::render::projection::globe_camera_for_view;
    let mut view = globe_view(Camera {
        center: LonLat {
            longitude: 10.0,
            latitude: 84.0,
        },
        zoom: 4.0,
    });
    let mut highest = 84.0_f64;
    for _ in 0..300 {
        assert!(view.drag_free_globe(Point2::new(400.0, 292.0), Point2::new(400.0, 300.0)));
        let center = globe_camera_for_view(&view)
            .unwrap()
            .screen_point_to_location(Point2::new(400.0, 300.0))
            .unwrap();
        highest = highest.max(center.latitude);
    }
    let center = globe_camera_for_view(&view)
        .unwrap()
        .screen_point_to_location(Point2::new(400.0, 300.0))
        .unwrap();
    assert!(highest > 89.9, "{highest}");
    assert!((center.longitude - 10.0).abs() > 90.0, "{center:?}");
}
