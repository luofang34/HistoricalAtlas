use crate::{navigation::Camera, scene::port::DisplayProjection};
use atlas_geo::{body::BodyId, coordinates::LonLat};

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
    view.set_navigation_mode(
        NavigationMode::FreeGlobe,
        &DisplayProjection::VerticalPerspective.into_maplibre(),
    )
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

#[cfg(not(target_arch = "wasm32"))]
fn nested_query_fixture() -> (serde_json::Value, [atlas_core::identity::EntityId; 4]) {
    use atlas_core::identity::EntityId;
    use serde_json::json;
    let county = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    let commune = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    let village = EntityId::from_key("earth/cn-gz/bijie/daqiao/xuhua-tun").unwrap();
    let river = EntityId::from_key("earth/cn-gz/bijie/luozhou-he").unwrap();
    let features = json!({
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[
                    [105.0, 27.0], [106.0, 27.0], [106.0, 28.0], [105.0, 28.0], [105.0, 27.0]
                ]]},
                "properties": {"entity_id": county, "geometry_role": "registered_boundary", "selected": false}
            },
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[
                    [105.25, 27.25], [105.45, 27.25], [105.45, 27.45], [105.25, 27.45], [105.25, 27.25]
                ]]},
                "properties": {"entity_id": commune, "geometry_role": "derived_boundary", "selected": false}
            },
            {
                "type": "Feature",
                "geometry": {"type": "LineString", "coordinates": [[105.38, 27.28], [105.42, 27.32]]},
                "properties": {"entity_id": river, "geometry_role": "registered_line", "selected": false}
            },
            {
                "type": "Feature",
                "geometry": {"type": "Point", "coordinates": [105.30, 27.30]},
                "properties": {"entity_id": village, "geometry_role": "location_estimate", "selected": false}
            }
        ]
    });
    (features, [village, river, commune, county])
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn query_rendered_features_picks_points_lines_and_nested_polygons_on_globe() {
    use crate::scene::{ScenePort, basemap::Manifest, renderer::MapRenderer, style_with_basemap};
    use maplibre::{coords::LatLon, render::projection::globe_camera_for_view};
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let instance = wgpu::Instance::default();
        let Ok(gpu) = egui_wgpu::RenderState::create(
            &Default::default(),
            &instance,
            None,
            egui_wgpu::RendererOptions::PREDICTABLE,
        )
        .await
        else {
            return;
        };
        let (points, [village, river, commune, county]) = nested_query_fixture();
        let basemap = Manifest::parse(br#"{"attribution":"Test","tiles":[]}"#).unwrap();
        let style = style_with_basemap(
            points.clone(),
            BodyId::Earth,
            DisplayProjection::VerticalPerspective,
            &basemap,
        )
        .unwrap();
        let mut renderer = MapRenderer::new(gpu, style, &basemap).unwrap();
        renderer.update(points, BodyId::Earth).unwrap();
        let focus = Camera {
            center: LonLat {
                longitude: 105.30,
                latitude: 27.30,
            },
            zoom: 10.0,
        };
        renderer
            .frame(egui::vec2(800.0, 600.0), 1.0, Some(focus))
            .unwrap();
        for _ in 0..80 {
            if !renderer.needs_redraw() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            renderer.frame(egui::vec2(800.0, 600.0), 1.0, None).unwrap();
        }
        assert!(!renderer.needs_redraw());
        let cam = globe_camera_for_view(renderer.map.view_state()).unwrap();
        let px = |lon: f64, lat: f64| -> [f64; 2] {
            let p = cam.location_to_screen(LatLon::new(lat, lon), 0.0);
            [p.x, p.y]
        };
        assert_eq!(
            renderer.pick(px(105.30, 27.30)).unwrap(),
            vec![village, commune, county]
        );
        assert_eq!(
            renderer.pick(px(105.40, 27.30)).unwrap(),
            vec![river, commune, county]
        );
        assert_eq!(
            renderer.pick(px(105.35, 27.40)).unwrap(),
            vec![commune, county]
        );
        assert_eq!(renderer.pick(px(105.48, 27.30)).unwrap(), vec![county]);
        renderer
            .frame(
                egui::vec2(800.0, 600.0),
                1.0,
                Some(Camera::for_body(BodyId::Earth)),
            )
            .unwrap();
        assert!(renderer.pick([5.0, 5.0]).unwrap().is_empty());
    });
}
