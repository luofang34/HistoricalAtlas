use super::*;

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
