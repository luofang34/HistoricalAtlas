use super::super::*;

#[test]
fn default_map_shows_drawn_places_and_requires_a_toggle_for_terrain_estimates() {
    let mut app =
        Application::new(Atlas::new(crate::catalog::tests::terrain_publication()).unwrap())
            .unwrap();
    assert_eq!(app.layer, MapLayer::MappedPlaces);
    assert!(app.camera_pending);
    let roles = |app: &Application, role: &str| {
        app.points().unwrap()["features"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|feature| feature["properties"]["geometry_role"] == role)
            .count()
    };
    assert!(roles(&app, "location_estimate") > 0);
    assert!(roles(&app, "registered_boundary") > 0);
    assert_eq!(roles(&app, "source_observation"), 0);
    assert_eq!(roles(&app, "derived_boundary"), 0);
    let id = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    assert!(scene::boundaries::frame(&app.atlas, id, app.map_size, false).is_none());
    app.derived_visible = true;
    assert_eq!(roles(&app, "derived_boundary"), 124);
    assert!(scene::boundaries::frame(&app.atlas, id, app.map_size, true).is_some());
    app.layer = MapLayer::Observations;
    assert!(roles(&app, "source_observation") > 0);
}

#[test]
fn selecting_a_table_anchor_in_a_drawn_commune_opens_its_measured_symbols() {
    let mut app = Application::new(crate::catalog::tests::atlas()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/zhijin/pingqiao-commune").unwrap();
    let before = app.camera.center;
    app.select(id, true);
    assert!(app.sheet.visible);
    assert_eq!(app.camera.center, before);
    let drawing = app.atlas.drawings(id)[0];
    assert_eq!(drawing.scan_page, 33);
    assert_eq!(drawing.places.len(), 36);
    assert_eq!(drawing.units.len(), 1);
}
