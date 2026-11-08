use super::Application;

impl Application {
    pub(super) fn map_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Source coordinate anchors · Drag to pan · Scroll to zoom");
        if self.dirty {
            let points = match self.points() {
                Ok(points) => points,
                Err(error) => {
                    self.error = Some(error.to_string());
                    return;
                }
            };
            if let Some(map) = &mut self.map {
                match map.update(points) {
                    Ok(()) => self.dirty = false,
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
        let size = ui.available_size().max(egui::vec2(1.0, 1.0));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        self.gestures(ui, &response);
        let Some(map) = &mut self.map else {
            return;
        };
        match map.frame(size, ui.ctx().pixels_per_point(), self.camera) {
            Ok(texture) => {
                ui.painter().image(
                    texture,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        if map.needs_redraw() {
            ui.ctx().request_repaint();
        }
        if response.clicked()
            && let Some(point) = response.interact_pointer_pos()
        {
            let relative = point - rect.min;
            match map.pick([f64::from(relative.x), f64::from(relative.y)]) {
                Ok(ids) => {
                    self.hits = ids;
                    if let Some(id) = self.hits.first().copied() {
                        self.select(id, false);
                    }
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }

    fn gestures(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if response.dragged() {
            let delta = ui.input(|input| input.pointer.delta());
            self.camera.pan(f64::from(delta.x), f64::from(delta.y));
        }
        if response.hovered() {
            let delta = ui.input(|input| input.smooth_scroll_delta.y);
            if delta != 0.0
                && let Some(position) = response.hover_pos()
            {
                let offset = position - response.rect.center();
                self.camera.zoom_at(
                    f64::from(delta) / 200.0,
                    [f64::from(offset.x), f64::from(offset.y)],
                );
            }
        }
    }
}
