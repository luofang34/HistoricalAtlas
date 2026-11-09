use super::*;

#[test]
fn native_font_inventory_keeps_configured_glyphs_and_reports_missing_assets() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("fonts.json"), br#"["Names.otf"]"#).unwrap();
    let context = egui::Context::default();
    assert!(matches!(
        configure_blocking(&context, directory.path()),
        Err(FontError::Io { .. })
    ));
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/AtlasCJK.otf"),
        directory.path().join("Names.otf"),
    )
    .unwrap();
    configure_blocking(&context, directory.path()).unwrap();
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        ui.label("毕节 ABC");
    });
    assert!(
        !context
            .tessellate(output.shapes, output.pixels_per_point)
            .is_empty()
    );
    output.textures_delta.clear();
    context
        .fonts_mut(|fonts| assert!(fonts.has_glyphs(&egui::FontId::proportional(14.), "毕节 ABC")));
    for value in [
        br#"["../Font.otf"]"#.as_slice(),
        br#"["Font.js"]"#,
        br#"["/Font.ttf"]"#,
    ] {
        assert!(names(value).is_err());
    }
}
