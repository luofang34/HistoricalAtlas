use super::Application;
use crate::scene::{MapLayer, ViewportPointer};
use atlas_geo::coordinates::LonLat;

impl Application {
    fn map_caption(&self) -> &'static str {
        if self.territory_visible {
            return "Territory nodes and place locations · Drag to rotate · Right- or Ctrl-drag to tilt · Scroll to zoom";
        }
        match self.layer {
            MapLayer::Observations => {
                "Printed coordinates and source-map boundaries · Drag to rotate · Right- or Ctrl-drag to tilt · Scroll to zoom"
            }
            MapLayer::MappedPlaces => {
                "Source-map places and traced boundaries · Drag to rotate · Right- or Ctrl-drag to tilt · Scroll to zoom"
            }
            MapLayer::SelectedTime => {
                "Time-selected source anchors · Drag to rotate · Right- or Ctrl-drag to tilt · Scroll to zoom"
            }
        }
    }

    pub(super) fn map_panel(&mut self, ui: &mut egui::Ui) {
        #[cfg(target_arch = "wasm32")]
        self.load_visible_streams(ui);
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
                    Ok(()) => {
                        self.dirty = false;
                        self.selection_dirty = true;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
        self.update_selection();
        let size = ui.available_size().max(egui::vec2(1.0, 1.0));
        self.map_size = [f64::from(size.x), f64::from(size.y)];
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let pointer = ViewportPointer::read(ui, &response);
        let on_controls = self
            .map
            .as_mut()
            .is_some_and(|map| map.pointer_controls(pointer));
        if !on_controls {
            self.gestures(ui, &response);
        }
        let Some(map) = &mut self.map else {
            return;
        };
        if map.advance_controls() {
            ui.ctx().request_repaint();
        }
        match map.frame(
            size,
            ui.ctx().pixels_per_point(),
            self.camera_pending.then_some(self.camera),
        ) {
            Ok(texture) => {
                if std::mem::take(&mut self.camera_pending) {
                    ui.ctx().request_repaint();
                }
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
            && !on_controls
            && let Some(point) = response.interact_pointer_pos()
        {
            self.pick(point - rect.min);
        }
        self.attribution(ui, rect);
    }

    fn pick(&mut self, pixel: egui::Vec2) {
        let Some(map) = &self.map else {
            return;
        };
        match map.pick([f64::from(pixel.x), f64::from(pixel.y)]) {
            Ok(ids) => {
                self.error = None;
                self.hits = ids;
                if let Some(id) = self.hits.first().copied() {
                    self.select(id, false);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn attribution(&self, ui: &egui::Ui, rect: egui::Rect) {
        if !self.basemap.has_body(self.body) {
            return;
        }
        let color = egui::Color32::from_rgb(220, 226, 219);
        let galley = ui.painter().layout(
            self.basemap.attribution(self.body),
            egui::FontId::proportional(11.0),
            color,
            (rect.width() - 16.0).max(1.0),
        );
        let position = rect.right_bottom() - egui::vec2(8.0, 8.0) - galley.size();
        ui.painter().galley(position, galley, color);
    }

    fn update_selection(&mut self) {
        if self.selection_dirty {
            match self.selection_features().and_then(|extra| {
                if let Some(map) = &mut self.map {
                    map.select(self.selected, extra)?;
                }
                Ok(())
            }) {
                Ok(()) => self.selection_dirty = false,
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }

    fn view_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Globe").clicked() {
                self.camera = crate::navigation::Camera::for_body(self.body);
                self.camera_pending = true;
                self.sheet.visible = false;
            }
            if ui.button("Data extent").clicked()
                && let Some(bounds) = self.data_extent()
            {
                self.camera.fit_bounds(bounds, self.map_size);
                self.camera_pending = true;
                self.sheet.visible = false;
            }
            self.source_drawings(ui);
            self.dirty |= ui
                .checkbox(&mut self.derived_visible, "Terrain estimates")
                .changed();
        });
    }

    pub(super) fn data_extent(&self) -> Option<[LonLat; 2]> {
        let points = if self.layer == MapLayer::SelectedTime {
            self.selected_time_extent_points()
        } else {
            self.present_extent_points()
        };
        let first = *points.first()?;
        Some(points.iter().fold([first, first], |[low, high], point| {
            [
                LonLat {
                    longitude: low.longitude.min(point.longitude),
                    latitude: low.latitude.min(point.latitude),
                },
                LonLat {
                    longitude: high.longitude.max(point.longitude),
                    latitude: high.latitude.max(point.latitude),
                },
            ]
        }))
    }

    fn selected_time_extent_points(&self) -> Vec<LonLat> {
        let mut points = Vec::new();
        for pos in self.selected_time.paleo.values() {
            if self.atlas.entity_body(pos.entity_id) == Some(self.body) {
                points.push(pos.reconstructed);
            }
        }
        if self.crustal_pins {
            for pos in self.selected_time.crustal_pins.values() {
                if self.atlas.entity_body(pos.entity_id) == Some(self.body) {
                    points.push(pos.reconstructed);
                }
            }
        }
        let active = |id| {
            self.atlas.entity_body(id) == Some(self.body)
                && self.selected_time.evidence.contains_key(&id)
                && !self.selected_time.is_excluded(id)
        };
        for record in &self.atlas.data.records {
            if active(record.entity_id)
                && !self.selected_time.paleo.contains_key(&record.id)
                && let Some(coord) = &record.observation.coordinate
            {
                points.push(coord.point);
            }
        }
        for entity in self
            .atlas
            .data
            .entities
            .iter()
            .filter(|e| e.body == self.body)
        {
            if let Some(bounds) = crate::scene::boundaries::extent(
                &self.atlas,
                entity.id,
                self.derived_visible,
                Some(&self.selected_time),
            ) {
                points.extend(bounds);
            }
        }
        points
    }

    fn present_extent_points(&self) -> Vec<LonLat> {
        let mut points = if self.layer == MapLayer::Observations {
            self.atlas
                .data
                .records
                .iter()
                .filter_map(|record| {
                    (self.atlas.entity_body(record.entity_id) == Some(self.body))
                        .then_some(record.observation.coordinate.as_ref()?.point)
                })
                .collect::<Vec<_>>()
        } else {
            self.atlas
                .data
                .estimates
                .iter()
                .filter_map(|estimate| {
                    if self.atlas.entity_body(estimate.entity_id) != Some(self.body) {
                        return None;
                    }
                    match estimate.value {
                        atlas_ledger::model::EstimateValue::MapLocation { point, .. } => {
                            Some(point)
                        }
                        _ => None,
                    }
                })
                .collect::<Vec<_>>()
        };
        for entity in self
            .atlas
            .data
            .entities
            .iter()
            .filter(|entity| entity.body == self.body)
        {
            if let Some(bounds) =
                crate::scene::boundaries::extent(&self.atlas, entity.id, self.derived_visible, None)
            {
                points.extend(bounds);
            }
        }
        if points.is_empty() {
            points.extend(self.atlas.data.records.iter().filter_map(|record| {
                (self.atlas.entity_body(record.entity_id) == Some(self.body))
                    .then_some(record.observation.coordinate.as_ref()?.point)
            }));
        }
        if points.is_empty()
            && let Some([w, s, e, n]) = self.atlas.body_shard_bounds(self.body)
        {
            points.push(LonLat {
                longitude: w,
                latitude: s,
            });
            points.push(LonLat {
                longitude: e,
                latitude: n,
            });
        }
        points
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
        let orbiting = response.dragged_by(egui::PointerButton::Secondary)
            || (response.dragged_by(egui::PointerButton::Primary)
                && ui.input(|input| input.modifiers.command));
        if orbiting {
            let delta = ui.input(|input| input.pointer.delta());
            match map.orbit(delta) {
                Ok(true) => ui.ctx().request_repaint(),
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "map orbit rejected"),
            }
        } else if response.dragged() {
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
