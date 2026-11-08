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
