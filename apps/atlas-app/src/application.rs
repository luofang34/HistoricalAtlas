//! Shared atlas interaction and evidence panels.

use crate::{
    catalog::Atlas,
    error::AppError,
    navigation::Camera,
    scene::{self, MapLayer, renderer::MapRenderer},
    selection::{SelectedTime, TimeControl},
};
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;

mod controls;
mod evidence;
mod map;

/// Application state independent of the windowing platform.
pub struct Application {
    atlas: Atlas,
    body: BodyId,
    time: TimeControl,
    selected_time: SelectedTime,
    selected: Option<EntityId>,
    search: String,
    hits: Vec<EntityId>,
    layer: MapLayer,
    camera: Camera,
    map: Option<MapRenderer>,
    dirty: bool,
    error: Option<String>,
}

impl Application {
    /// Start with reviewed Earth observations and the 1982 time selection.
    pub fn new(atlas: Atlas) -> Result<Self, AppError> {
        let time = TimeControl::default();
        let selected_time = SelectedTime::query(&atlas, &time)?;
        Ok(Self {
            atlas,
            body: BodyId::Earth,
            time,
            selected_time,
            selected: None,
            search: String::new(),
            hits: Vec::new(),
            layer: MapLayer::Observations,
            camera: Camera::for_body(BodyId::Earth),
            map: None,
            dirty: true,
            error: None,
        })
    }

    /// Attach the GPU renderer after the window has a drawing surface.
    pub fn attach(&mut self, gpu: egui_wgpu::RenderState) -> Result<(), AppError> {
        self.map = Some(MapRenderer::new(gpu, scene::style(self.points())?)?);
        self.dirty = false;
        Ok(())
    }

    /// Draw a complete frame of the common desktop and browser interface.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("controls").show(ui, |ui| self.controls(ui));
        egui::Panel::left("places")
            .default_size(240.0)
            .show(ui, |ui| self.search_panel(ui));
        egui::Panel::right("evidence")
            .default_size(340.0)
            .show(ui, |ui| self.evidence(ui));
        egui::CentralPanel::default().show(ui, |ui| self.map_panel(ui));
    }

    fn select(&mut self, id: EntityId, focus: bool) {
        self.selected = Some(id);
        self.dirty = true;
        if focus
            && let Some(point) = self.atlas.entry(id).and_then(|entry| {
                entry
                    .records
                    .iter()
                    .find_map(|record| record.observation.coordinate.as_ref())
            })
        {
            self.camera.center = point.point;
            self.camera.zoom = if self.body == BodyId::Earth {
                12.0
            } else {
                5.0
            };
        }
    }

    fn points(&self) -> serde_json::Value {
        scene::features(
            &self.atlas,
            self.body,
            &self.selected_time,
            self.layer,
            self.selected,
        )
    }

    fn apply_time(&mut self) {
        match SelectedTime::query(&self.atlas, &self.time) {
            Ok(time) => {
                self.selected_time = time;
                self.dirty = true;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn label(&self, id: EntityId) -> String {
        if self.layer == MapLayer::SelectedTime
            && let Some(names) = self
                .selected_time
                .names
                .get(&id)
                .filter(|names| !names.is_empty())
        {
            return names.join(" / ");
        }
        self.atlas.label(id)
    }
}

/// Install the publication's Chinese glyph subset alongside egui's default fonts.
pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Atlas CJK".into(),
        egui::FontData::from_static(include_bytes!("../assets/AtlasCJK.otf")).into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("Atlas CJK".into());
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests;
