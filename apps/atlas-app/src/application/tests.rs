use super::*;

mod sheet;

#[test]
fn selecting_a_dated_boundary_updates_only_selection_and_frames_visible_geometry() {
    let atlas = Atlas::new(crate::scene::boundaries::tests::lunar_publication()).unwrap();
    let id = atlas.data.entities[0].id;
    let mut app = Application::new(atlas).unwrap();
    app.body = BodyId::Moon;
    app.layer = MapLayer::SelectedTime;
    app.time.mode = crate::selection::TimeMode::Ticks;
    app.time.value = "14".into();
    app.apply_time();
    let observations = app.points().unwrap();
    app.dirty = false;
    app.selection_dirty = false;
    app.select(id, true);
    assert!(!app.dirty);
    assert!(app.selection_dirty);
    assert_eq!(app.points().unwrap(), observations);
    assert_eq!(app.camera.center.longitude, 1.5);
    assert_eq!(app.camera.center.latitude, 1.5);
    app.time.value = "20".into();
    app.apply_time();
    assert!(app.focus_point(id).is_none());
    assert!(
        app.points().unwrap()["features"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn registered_boundary_search_focuses_geography_and_displays_source_method() {
    let id = EntityId::from_key("earth/cn-gz/zhijin/ertang-commune").unwrap();
    let mut publication = crate::scene::boundaries::tests::publication();
    let boundary = &mut publication.entries.get_mut(&id).unwrap().boundaries[0];
    boundary.datum = atlas_geo::coordinates::Datum::Unspecified;
    boundary.registration_allowance_m = Some(4500.);
    let atlas = Atlas::new(publication).unwrap();
    let expected = crate::scene::boundaries::center(&atlas, id, false, None).unwrap();
    let mut app = Application::new(atlas).unwrap();
    app.select(id, true);
    assert_eq!(app.camera.center, expected);
    assert!(!app.sheet.visible);
    let context = egui::Context::default();
    configure_fonts(&context);
    let output = date_frame(&context, &mut app, Vec::new());
    for label in [
        "Registered map boundary",
        "Source method: Compiled from place locations",
        "Datum: Unspecified",
        "Registration allowance: 4.5 km",
        "Effective dates: unknown",
    ] {
        assert!(
            output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::epaint::Shape::Text(text) if text.galley.job.text == label)),
            "{label}"
        );
    }
    let count = |app: &Application| {
        app.points().unwrap()["features"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| {
                f["properties"]["geometry_role"] == "registered_boundary"
                    && f["properties"]["entity_id"] == id.0.to_string()
            })
            .count()
    };
    assert_eq!(count(&app), 1);
    app.layer = MapLayer::MappedPlaces;
    app.select(id, true);
    assert_eq!(app.camera.center, expected);
    assert_eq!(count(&app), 1);
    app.layer = MapLayer::SelectedTime;
    assert_eq!(count(&app), 0);
    assert!(app.focus_point(id).is_none());
}

#[test]
fn mapped_place_view_keeps_registered_locations_and_frames_the_reconstruction() {
    let atlas = Atlas::new(crate::catalog::tests::terrain_publication()).unwrap();
    let mut app = Application::new(atlas).unwrap();
    app.derived_visible = true;
    let id = EntityId::from_key("earth/cn-gz/bijie/daqiao-commune").unwrap();
    app.select(id, true);
    let table = app.camera.center;
    app.layer = MapLayer::MappedPlaces;
    app.select(id, true);
    assert_eq!(app.camera.center, table);
    assert_eq!(
        app.camera.center,
        scene::boundaries::center(&app.atlas, id, true, None).unwrap()
    );
    let collection = app.points().unwrap();
    let features = collection["features"].as_array().unwrap();
    let selected: Vec<_> = features
        .iter()
        .filter(|f| {
            f["properties"]["entity_id"] == id.0.to_string()
                && f["properties"]["geometry_role"] == "location_estimate"
        })
        .collect();
    assert_eq!(selected.len(), 2);
    for feature in &selected {
        assert_eq!(feature["properties"]["selected"], false);
        assert_eq!(feature["properties"]["geometry_role"], "location_estimate");
        assert_eq!(feature["properties"]["datum"], "unspecified");
    }
    assert!(selected.iter().any(
        |f| f["properties"]["name"] == "大桥公社" && f["properties"]["uncertainty_m"] == 4500.0
    ));
    assert!(
        selected.iter().any(
            |f| f["properties"]["name"] == "大桥" && f["properties"]["uncertainty_m"] == 3500.0
        )
    );
    let point = app.focus_point(id).unwrap();
    assert!(
        selected.iter().any(|f| f["geometry"]["coordinates"]
            == serde_json::json!([point.longitude, point.latitude]))
    );
    let wuliping = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    assert_eq!(app.label(wuliping), "五里坪公社");
    app.body = BodyId::Moon;
    assert!(
        app.points().unwrap()["features"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    app.body = BodyId::Earth;
    app.layer = MapLayer::SelectedTime;
    app.time.mode = crate::selection::TimeMode::MillionYears;
    app.time.value = "240".into();
    app.apply_time();
    assert!(
        app.points().unwrap()["features"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn county_search_fits_the_outline_to_the_map_viewport() {
    let mut app = Application::new(crate::catalog::tests::atlas()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    assert!(app.atlas.search("毕节县", BodyId::Earth).contains(&id));
    app.map_size = [800.0, 600.0];
    app.select(id, true);
    assert!((8.0..10.0).contains(&app.camera.zoom));
    assert!(!app.sheet.visible);
    let wide = app.camera.zoom;
    app.map_size = [320.0, 600.0];
    app.select(id, true);
    assert!(app.camera.zoom < wide);
    let collection = app.points().unwrap();
    let boundary = collection["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| {
            f["properties"]["geometry_role"] == "registered_boundary"
                && f["properties"]["entity_id"] == id.0.to_string()
        })
        .unwrap();
    assert_eq!(boundary["properties"]["selected"], false);
    assert_eq!(boundary["properties"]["scan_page"], 658);
}

#[test]
fn street_search_displays_source_extent_without_focusing_an_invented_point() {
    let atlas = crate::catalog::tests::atlas();
    let id = EntityId::from_key("earth/cn-gz/bijie/urban/weining-west-road").unwrap();
    assert!(atlas.search("半边街", BodyId::Earth).contains(&id));
    let mut app = Application::new(atlas).unwrap();
    let center = app.camera.center;
    let zoom = app.camera.zoom;
    app.select(id, true);
    assert_eq!(app.camera.center, center);
    assert_eq!(app.camera.zoom, zoom);
    let context = egui::Context::default();
    configure_fonts(&context);
    let output = date_frame(&context, &mut app, Vec::new());
    assert!(
        output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::epaint::Shape::Text(text) if text.galley.job.text=="Extent: 东起威宁路，西止翠屏路"))
    );
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::epaint::Shape::Text(text) if text.galley.job.text=="Former name: 半边街")));
    assert!(
        !app.points().unwrap()["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|feature| feature["properties"]["entity_id"] == id.0.to_string())
    );
}

#[test]
fn territory_toggle_adds_links_and_respects_the_applied_hierarchy_mode() {
    let mut app = Application::new(crate::catalog::tests::atlas()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    app.select(id, false);
    let roles = |app: &Application| {
        app.selection_features()
            .unwrap()
            .iter()
            .filter(|feature| feature["properties"]["geometry_role"] == "territory_link")
            .count()
    };
    assert_eq!(roles(&app), 0);
    app.territory_visible = true;
    assert!(roles(&app) > 0);
    app.layer = MapLayer::SelectedTime;
    assert_eq!(roles(&app), 0);
    app.layer = MapLayer::Observations;
    app.territory_visible = false;
    assert_eq!(roles(&app), 0);
}

#[test]
fn unresolved_coordinate_is_visible_in_the_evidence_card_without_a_map_point() {
    let atlas = crate::catalog::tests::atlas();
    let id = atlas.search("银河大队", BodyId::Earth)[0];
    let mut app = Application::new(atlas).unwrap();
    app.select(id, true);
    let context = egui::Context::default();
    configure_fonts(&context);
    let output = date_frame(&context, &mut app, Vec::new());
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::epaint::Shape::Text(text) if text.galley.job.text.contains("185°20′"))));
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::epaint::Shape::Text(text) if text.galley.job.text.contains("exceeds 180°"))));
    assert!(
        !app.points().unwrap()["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|feature| feature["properties"]["entity_id"] == id.0.to_string())
    );
}

#[test]
fn clicking_an_era_date_queries_its_converted_midpoint() {
    let atlas = crate::catalog::tests::atlas();
    let id = atlas.search("禄州", BodyId::Earth)[0];
    let expected = atlas.entry(id).unwrap().records[0]
        .source_date
        .as_ref()
        .unwrap()
        .era_conversion
        .as_ref()
        .unwrap()
        .to_estimate(0)
        .unwrap()
        .nominal;
    let mut app = Application::new(atlas).unwrap();
    app.select(id, false);
    let context = egui::Context::default();
    configure_fonts(&context);
    let output = date_frame(&context, &mut app, Vec::new());
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "Show date on timeline" => {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap();
    for pressed in [true, false] {
        date_frame(
            &context,
            &mut app,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
    }
    assert_eq!(app.selected_time.at, expected);
    assert_eq!(app.time.mode, crate::selection::TimeMode::Ticks);
    assert_eq!(
        app.selected_time.evidence[&id]["origins"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[cfg(test)]
fn date_frame(
    context: &egui::Context,
    app: &mut Application,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
    output
}

#[test]
fn common_ui_renders_source_evidence_and_chinese_glyphs() {
    let atlas = crate::catalog::tests::atlas();
    let id = atlas.search("翠屏公社", BodyId::Earth)[0];
    let mut app = Application::new(atlas).unwrap();
    app.select(id, true);
    let context = egui::Context::default();
    configure_fonts(&context);
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440.0, 900.0),
            )),
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    assert!(
        !context
            .tessellate(output.shapes, output.pixels_per_point)
            .is_empty()
    );
    output.textures_delta.clear();
    context.fonts_mut(|fonts| {
        for record in &app.atlas.data.records {
            assert!(
                fonts.has_glyphs(&egui::FontId::proportional(14.0), &record.observation.name),
                "{}",
                record.observation.name
            );
        }
    });
    app.time.value = "invalid".into();
    let epoch = app.selected_time.at;
    app.apply_time();
    assert_eq!(app.selected_time.at, epoch);
    assert!(app.error.is_some());
}

#[cfg(test)]
fn click_label(context: &egui::Context, app: &mut Application, label: &str) {
    let output = date_frame(context, app, Vec::new());
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap();
    for pressed in [true, false] {
        date_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
    }
}

#[cfg(test)]
fn event_markers(app: &Application) -> usize {
    app.points().unwrap()["features"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|feature| feature["properties"]["geometry_role"] == "external_event")
        .count()
}

#[test]
fn event_controls_navigate_time_and_place_and_toggle_only_external_markers() {
    let atlas = crate::catalog::tests::atlas();
    let records = atlas.data.records.len();
    let mut app = Application::new(atlas).unwrap();
    app.load_overlay(crate::overlay::OverlayInput {
        origin: "https://events.example/apollo.json".into(),
        content: Ok(crate::overlay::tests::example().into()),
    });
    let context = egui::Context::default();
    configure_fonts(&context);
    assert_eq!(event_markers(&app), 0);
    click_label(&context, &mut app, "Eagle lands at Tranquility Base");
    assert_eq!(app.body, BodyId::Moon);
    assert_eq!(
        app.selected,
        Some(EntityId::from_key("moon/tranquility-base").unwrap())
    );
    assert_eq!(app.selected_time.at.ticks(), -960_910_900_241);
    assert_eq!(event_markers(&app), 1);
    click_label(&context, &mut app, "Show event markers");
    assert_eq!(event_markers(&app), 0);
    assert_eq!(
        app.points().unwrap()["features"].as_array().unwrap().len(),
        3
    );
    click_label(&context, &mut app, "Show event markers");
    assert_eq!(event_markers(&app), 1);
    app.time.selection = atlas_ledger::timeline::TimeSelection::Certain;
    assert_eq!(event_markers(&app), 1);
    app.apply_time();
    assert_eq!(event_markers(&app), 0);
    app.load_overlay(crate::overlay::OverlayInput {
        origin: "https://events.example/broken.json".into(),
        content: Err("HTTP 404".into()),
    });
    assert!(app.overlay.source.is_some());
    assert_eq!(app.atlas.data.records.len(), records);
    let output = date_frame(&context, &mut app, Vec::new());
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::epaint::Shape::Text(text) if text.galley.job.text.contains("HTTP 404"))));
}

#[test]
fn external_event_description_wraps_within_the_evidence_panel() {
    let mut app = Application::new(crate::catalog::tests::atlas()).unwrap();
    app.load_overlay(crate::overlay::OverlayInput {
        origin: "Local Apollo 11 fixture".into(),
        content: Ok(crate::overlay::tests::example().into()),
    });
    let context = egui::Context::default();
    configure_fonts(&context);
    click_label(&context, &mut app, "Eagle lands at Tranquility Base");
    let output = date_frame(&context, &mut app, Vec::new());
    let description = app.overlay.source.as_ref().unwrap().package().events[0]
        .description
        .as_ref()
        .unwrap();
    let galley = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if &text.galley.job.text == description => {
                Some(&text.galley)
            }
            _ => None,
        })
        .unwrap();
    assert!(galley.size().x < 340.0, "{}", galley.size().x);
    assert!(galley.rows.len() > 1);
}
