use super::*;

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
