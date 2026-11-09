use super::Application;

impl Application {
    fn map_caption(&self) -> &'static str {
        if self.territory_visible {
            return "Territory nodes and place locations · Drag to rotate · Scroll to zoom";
        }
        match self.layer {
            crate::scene::MapLayer::Observations => {
                "Printed coordinates and source-map boundaries · Drag to rotate · Scroll to zoom"
            }
            crate::scene::MapLayer::MappedPlaces => {
                "Source-map places and traced boundaries · Drag to rotate · Scroll to zoom"
            }
            crate::scene::MapLayer::SelectedTime => {
                "Time-selected source anchors · Drag to rotate · Scroll to zoom"
            }
        }
    }

    pub(super) fn map_panel(&mut self, ui: &mut egui::Ui) {
        self.view_controls(ui);
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
        self.map_size = [f64::from(size.x), f64::from(size.y)];
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        self.gestures(ui, &response);
        let Some(map) = &mut self.map else {
            return;
        };
        match map.frame(
            size,
            ui.ctx().pixels_per_point(),
            self.camera_pending.then_some(self.camera),
        ) {
            Ok(texture) => {
                self.camera_pending = false;
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
                egui::Color32::from_rgb(220, 226, 219),
            );
        }
    }

    fn view_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Globe").clicked() {
                self.camera = crate::navigation::Camera::for_body(self.body);
                self.camera_pending = true;
                self.sheet.visible = false;
            }
            if self.body == atlas_geo::body::BodyId::Earth && ui.button("Guizhou").clicked() {
                self.camera.center = atlas_geo::coordinates::LonLat {
                    longitude: 105.6,
                    latitude: 26.9,
                };
                self.camera.zoom = 8.0;
                self.camera_pending = true;
                self.sheet.visible = false;
            }
            self.source_drawings(ui);
            self.dirty |= ui
                .checkbox(&mut self.derived_visible, "Terrain estimates")
                .changed();
        });
    }

    fn gestures(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if self.camera_pending {
            return;
        }
        let Some(map) = &mut self.map else {
            return;
        };
        let Some(position) = response
            .interact_pointer_pos()
            .or_else(|| response.hover_pos())
        else {
            return;
        };
        let pixel = position - response.rect.min;
        if response.dragged() {
            let delta = ui.input(|input| input.pointer.delta());
            if map.drag(pixel - delta, pixel) {
                ui.ctx().request_repaint();
            }
        }
        if response.hovered() {
            let delta = ui.input(|input| input.smooth_scroll_delta.y);
            if delta != 0.0 && map.zoom(pixel, f64::from(delta) / 200.0) {
                ui.ctx().request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests;
