use super::*;

#[test]
fn selecting_unregistered_place_opens_the_source_drawing() {
    let mut app = Application::new(crate::catalog::tests::atlas()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/zhijin/ertang/chenjia-qiao").unwrap();
    let center = app.camera.center;
    app.select(id, true);
    assert!(app.sheet.visible);
    assert_eq!(app.camera.center, center);
    let context = egui::Context::default();
    configure_fonts(&context);
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 900.0),
            )),
            ..Default::default()
        },
        |ui| app.map_panel(ui),
    );
    output.textures_delta.clear();
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Path(path) if path.points.len() == 122)));
    for name in ["陈家桥", "二塘", "大坡上", "保底冲"] {
        assert!(
            output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == name)),
            "{name}"
        );
    }
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text.contains("Sheet coordinates"))));
    app.apply_time();
    assert!(!app.sheet.visible);
}
