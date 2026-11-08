use super::Application;

impl Application {
    fn map_caption(&self) -> &'static str {
        if self.territory_visible {
            return "Territory nodes and place locations · Drag to pan · Scroll to zoom";
        }
        match self.layer {
            crate::scene::MapLayer::Observations => {
                "Source coordinate anchors · Drag to pan · Scroll to zoom"
            }
            crate::scene::MapLayer::MappedPlaces => {
                "Map-derived locations · Drag to pan · Scroll to zoom"
            }
            crate::scene::MapLayer::SelectedTime => {
                "Time-selected source anchors · Drag to pan · Scroll to zoom"
            }
        }
    }

    pub(super) fn map_panel(&mut self, ui: &mut egui::Ui) {
        if self.sheet_panel(ui) {
            return;
        }
        ui.label(self.map_caption());
        if self.dirty {
            let points = match self.points() {
                Ok(points) => points,
                Err(error) => {
                    self.error = Some(error.to_string());
                    return;
                }
            };
            if let Some(map) = &mut self.map {
                match map.update(points, self.body) {
                    Ok(()) => self.dirty = false,
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
        let size = ui.available_size().max(egui::vec2(1.0, 1.0));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        self.gestures(ui, &response);
        self.camera.fit_height(f64::from(size.y));
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
        if self.body == atlas_geo::body::BodyId::Earth {
            let text = "Natural Earth II · Terrain: Mapzen / USGS";
            let position = rect.right_bottom() - egui::vec2(8.0, 8.0);
            ui.painter().text(
                position,
                egui::Align2::RIGHT_BOTTOM,
                text,
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(40, 45, 38),
            );
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
