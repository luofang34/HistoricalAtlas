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

#[test]
fn fitted_qimo_drawing_keeps_all_settlement_labels_readable() {
    let mut app = Application::new(crate::catalog::tests::atlas()).unwrap();
    let id = EntityId::from_key("earth/cn-gz/zhijin/qimo/jibu").unwrap();
    app.select(id, true);
    let drawing = app.atlas.drawings(id)[0];
    let names: Vec<_> = crate::scene::sheet::points(&app.atlas, drawing)
        .into_iter()
        .map(|p| p.names[0].clone())
        .collect();
    let context = egui::Context::default();
    configure_fonts(&context);
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 800.0),
            )),
            ..Default::default()
        },
        |ui| app.map_panel(ui),
    );
    output.textures_delta.clear();
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if names.contains(&text.galley.job.text) => Some((
                &text.galley.job.text,
                text.galley.rect.translate(text.pos.to_vec2()),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(labels.len(), 51);
    for (index, (name, bounds)) in labels.iter().enumerate() {
        for (other, rect) in labels.iter().skip(index + 1) {
            assert!(
                !bounds.intersects(*rect),
                "{name} {bounds:?} overlaps {other} {rect:?}"
            );
        }
    }
}
