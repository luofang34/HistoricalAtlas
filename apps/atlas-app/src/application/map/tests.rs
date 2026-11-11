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

struct MockScene {
    hits: Vec<EntityId>,
    drags: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl ScenePort for MockScene {
    fn update(
        &mut self,
        _: serde_json::Value,
        _: BodyId,
    ) -> Result<(), crate::scene::renderer::RenderError> {
        Ok(())
    }
    fn select(
        &mut self,
        _: Option<EntityId>,
        _: Vec<serde_json::Value>,
    ) -> Result<(), crate::scene::renderer::RenderError> {
        Ok(())
    }
    fn frame(
        &mut self,
        _: egui::Vec2,
        _: f32,
        _: Option<Camera>,
    ) -> Result<egui::TextureId, crate::scene::renderer::RenderError> {
        Ok(egui::TextureId::Managed(0))
    }
    fn pick(&self, _: [f64; 2]) -> Result<Vec<EntityId>, crate::scene::renderer::RenderError> {
        Ok(self.hits.clone())
    }
    fn detail_bounds(&self) -> Option<[f64; 4]> {
        None
    }
    fn pointer_controls(&mut self, _: crate::scene::ViewportPointer) -> bool {
        false
    }
    fn advance_controls(&mut self) -> bool {
        false
    }
    fn drag(&mut self, _: egui::Vec2, _: egui::Vec2) -> bool {
        self.drags.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        true
    }
    fn orbit(&mut self, _: egui::Vec2) -> Result<bool, crate::scene::renderer::RenderError> {
        Ok(false)
    }
    fn zoom(&mut self, _: egui::Vec2, _: f64) -> bool {
        false
    }
    fn needs_redraw(&self) -> bool {
        false
    }
}

#[test]
fn clicking_map_viewport_ignores_tremor_expands_hierarchy_and_shows_original_source() {
    let atlas = crate::catalog::tests::atlas();
    let county = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    let commune = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    let drags = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut app = Application::new(atlas).unwrap();
    app.search = "stale-filter".into();
    app.map = Some(Box::new(MockScene {
        hits: vec![commune, county],
        drags: drags.clone(),
    }));
    let context = egui::Context::default();
    crate::application::configure_fonts_blocking(&context).unwrap();
    let run = |app: &mut Application, time: f64, events: Vec<egui::Event>| {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0));
        let input = egui::RawInput {
            screen_rect: Some(rect),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut out = context.run_ui(input, |ui| app.ui(ui));
        out.textures_delta.clear();
        out
    };
    run(&mut app, 0.0, Vec::new());
    let press = egui::pos2(720.0, 450.0);
    let tremor = egui::pos2(722.0, 451.0);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    run(
        &mut app,
        0.1,
        vec![egui::Event::PointerMoved(press), button(press, true)],
    );
    run(&mut app, 0.4, vec![egui::Event::PointerMoved(tremor)]);
    run(&mut app, 0.7, vec![button(tremor, false)]);
    assert_eq!(drags.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(app.selected, Some(commune));
    assert_eq!(app.hits, vec![commune, county]);
    assert!(app.search.is_empty());
    let out = run(&mut app, 0.8, Vec::new());
    let texts: Vec<&str> = out
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => Some(text.galley.job.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"At this map point"));
    assert!(texts.contains(&"Source hierarchy"));
    assert!(texts.contains(&"Source observations"));
}
