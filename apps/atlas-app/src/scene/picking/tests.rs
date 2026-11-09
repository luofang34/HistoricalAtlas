use super::*;

#[cfg(test)]
fn camera() -> GlobeCameraState {
    use maplibre::{
        coords::{WorldCoords, Zoom},
        render::{
            projection::globe_camera_for_view,
            view_state::{NavigationMode, ViewState},
        },
        window::PhysicalSize,
    };
    let zoom = Zoom::new(2.0);
    let mut view = ViewState::new(
        PhysicalSize::new(800, 600).unwrap(),
        WorldCoords::from_lat_lon(LatLon::new(0.0, 0.0), zoom),
        zoom,
        cgmath::Deg(0.0),
        cgmath::Deg(36.87),
    );
    view.set_globe_orbits_center(true);
    view.set_navigation_mode(NavigationMode::FreeGlobe, &crate::navigation::PROJECTION)
        .unwrap();
    globe_camera_for_view(&view).unwrap()
}

#[cfg(test)]
fn feature(key: &str, geometry: Value) -> Value {
    json!({"type":"Feature", "geometry":geometry, "properties":{"entity_id": EntityId::from_key(key).unwrap(), "selected":false}})
}

#[test]
fn globe_pick_retains_coincident_places_and_excludes_the_far_side_and_sky() {
    let features: Vec<_> = [("a", 0.0), ("b", 0.0), ("hidden", 180.0)]
        .into_iter()
        .map(|(key, lon)| feature(key, json!({"type":"Point", "coordinates":[lon,0.0]})))
        .collect();
    let index = Picking::new(&json!({"features":features})).unwrap();
    let camera = camera();
    let hits = index.pick(&camera, Point2::new(400.0, 300.0));
    assert_eq!(hits.len(), 2);
    assert!(hits.contains(&EntityId::from_key("a").unwrap()));
    assert!(hits.contains(&EntityId::from_key("b").unwrap()));
    assert!(index.pick(&camera, Point2::new(0.0, 0.0)).is_empty());
}

#[test]
fn globe_boundary_pick_respects_polygon_holes() {
    let shape = json!({"type":"Polygon", "coordinates":[[[-20,-20],[20,-20],[20,20],[-20,20],[-20,-20]],[[-2,-2],[-2,2],[2,2],[2,-2],[-2,-2]]]});
    let index = Picking::new(&json!({"features":[feature("region",shape)]})).unwrap();
    let camera = camera();
    assert!(index.pick(&camera, Point2::new(400.0, 300.0)).is_empty());
    let pixel = camera.location_to_screen(LatLon::new(0.0, 10.0), 0.0);
    assert_eq!(
        index.pick(&camera, pixel),
        [EntityId::from_key("region").unwrap()]
    );
}

#[test]
fn selection_copies_only_one_entity_and_leaves_the_source_unchanged() {
    let source = json!({"features":[feature("a",json!({"type":"Point","coordinates":[0,0]})),feature("b",json!({"type":"Point","coordinates":[1,1]}))]});
    let before = source.clone();
    let index = Picking::new(&source).unwrap();
    let selected = index.selection(Some(EntityId::from_key("a").unwrap()), Vec::new());
    assert_eq!(selected["features"].as_array().unwrap().len(), 1);
    assert_eq!(selected["features"][0]["properties"]["selected"], true);
    assert_eq!(source, before);
    assert!(
        index.selection(None, Vec::new())["features"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
