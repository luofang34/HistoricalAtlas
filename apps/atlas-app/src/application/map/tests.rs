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
    assert!(scene::boundaries::frame(&app.atlas, id, app.map_size, false, None).is_none());
    app.derived_visible = true;
    assert_eq!(roles(&app, "derived_boundary"), 124);
    assert!(scene::boundaries::frame(&app.atlas, id, app.map_size, true, None).is_some());
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
    assert_eq!(drawing.scan_page, Some(33));
    assert_eq!(drawing.places.len(), 36);
    assert_eq!(drawing.units.len(), 1);
}

#[test]
fn deep_time_focus_point_and_data_extent_follow_reconstructed_plate_positions() {
    let mut app = Application::new(crate::catalog::tests::atlas()).unwrap();
    let fauna = EntityId::from_key("earth/cn-gz/panxian/yangjuan-fauna").unwrap();
    app.layer = MapLayer::Observations;
    let present = app.focus_point(fauna).unwrap();

    app.layer = MapLayer::SelectedTime;
    app.time.mode = crate::selection::TimeMode::MillionYears;
    app.time.value = "240".into();
    app.apply_time();

    let reconstructed = app.focus_point(fauna).unwrap();
    assert!((present.longitude - reconstructed.longitude).abs() > 1.0);
    app.select(fauna, true);
    assert_eq!(app.camera.center, reconstructed);

    let [low, high] = app.data_extent().unwrap();
    assert!((low.longitude - reconstructed.longitude).abs() < 1e-12);
    assert!((high.longitude - reconstructed.longitude).abs() < 1e-12);
}

#[test]
fn data_extent_separates_source_maps_from_printed_coordinates() {
    let mut app = Application::new(crate::catalog::tests::atlas()).unwrap();
    app.layer = MapLayer::MappedPlaces;
    let [mapped_low, mapped_high] = app.data_extent().unwrap();
    assert!(mapped_low.latitude > 26.5);
    assert!(mapped_high.latitude < 28.0);

    app.layer = MapLayer::Observations;
    let [obs_low, obs_high] = app.data_extent().unwrap();
    assert!(obs_low.latitude < 25.6);
    assert!(obs_high.latitude > 27.5);
}
