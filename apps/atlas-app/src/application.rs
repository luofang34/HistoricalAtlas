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
mod dates;
mod evidence;
mod map;
mod overlay;
mod sheet;

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
    territory_visible: bool,
    camera: Camera,
    map_size: [f64; 2],
    map: Option<MapRenderer>,
    dirty: bool,
    error: Option<String>,
    overlay: overlay::OverlayState,
    sheet: sheet::SheetView,
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
            territory_visible: false,
            camera: Camera::for_body(BodyId::Earth),
            map_size: [800.0, 600.0],
            map: None,
            dirty: true,
            error: None,
            overlay: overlay::OverlayState::default(),
            sheet: sheet::SheetView::default(),
        })
    }

    /// Attach the GPU renderer after the window has a drawing surface.
    pub fn attach(&mut self, gpu: egui_wgpu::RenderState) -> Result<(), AppError> {
        self.map = Some(MapRenderer::new(
            gpu,
            scene::style(
                self.points()?,
                self.body,
                maplibre::projection::ProjectionType::Mercator,
            )?,
        )?);
        self.dirty = false;
        Ok(())
    }

    /// Draw a complete frame of the common desktop and browser interface.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("controls").show(ui, |ui| self.controls(ui));
        egui::Panel::left("places")
            .default_size(240.0)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                self.overlay_panel(ui);
                self.search_panel(ui);
            });
        egui::Panel::right("evidence")
            .default_size(340.0)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                self.evidence(ui);
            });
        egui::CentralPanel::default().show(ui, |ui| self.map_panel(ui));
    }

    fn select(&mut self, id: EntityId, focus: bool) {
        self.selected = Some(id);
        self.dirty = true;
        if focus && !self.atlas.drawings(id).is_empty() && self.focus_point(id).is_none() {
            self.sheet = sheet::SheetView::open();
        }
        if focus && let Some(point) = self.focus_point(id) {
            self.camera.center = point;
            self.camera.zoom = if self.body == BodyId::Earth {
                12.0
            } else {
                5.0
            };
        }
        if focus
            && self.layer != MapLayer::SelectedTime
            && let Some(camera) = scene::boundaries::frame(&self.atlas, id, self.map_size)
        {
            self.camera = camera;
            self.sheet.visible = false;
        }
    }

    fn focus_point(&self, id: EntityId) -> Option<atlas_geo::coordinates::LonLat> {
        let entry = self.atlas.entry(id)?;
        if self.layer == MapLayer::MappedPlaces {
            return entry
                .estimates
                .iter()
                .find_map(|estimate| match estimate.value {
                    atlas_ledger::model::EstimateValue::MapLocation { point, .. } => Some(point),
                    _ => None,
                })
                .or_else(|| scene::boundaries::center(&self.atlas, id));
        }
        entry
            .records
            .iter()
            .find_map(|record| record.observation.coordinate.as_ref().map(|c| c.point))
            .or_else(|| {
                (self.layer != MapLayer::SelectedTime)
                    .then(|| scene::boundaries::center(&self.atlas, id))
                    .flatten()
            })
    }

    fn points(&self) -> Result<serde_json::Value, AppError> {
        let mut collection = scene::features(
            &self.atlas,
            self.body,
            &self.selected_time,
            self.layer,
            self.selected,
        );
        if let Some(features) = collection
            .get_mut("features")
            .and_then(serde_json::Value::as_array_mut)
        {
            features.extend(scene::boundaries::features(
                &self.atlas,
                self.body,
                self.layer,
                self.selected,
            )?);
        }
        if self.territory_visible
            && let Some(id) = self.selected
            && let Some(features) = collection
                .get_mut("features")
                .and_then(serde_json::Value::as_array_mut)
        {
            features.extend(scene::territories::features(
                &self.atlas,
                id,
                &self.selected_time,
                self.layer,
            )?);
        }
        if self.overlay.visible
            && let Some(overlay) = &self.overlay.source
            && let Some(features) = collection
                .get_mut("features")
                .and_then(serde_json::Value::as_array_mut)
        {
            features.extend(overlay.features(
                &self.atlas,
                self.body,
                &self.selected_time,
                self.selected,
            )?);
        }
        Ok(collection)
    }

    fn apply_time(&mut self) {
        self.sheet.visible = false;
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
        if self.layer == MapLayer::MappedPlaces
            && let Some(entry) = self.atlas.entry(id)
            && let Some(record_id) = entry.estimates.iter().find_map(|e| match e.value {
                atlas_ledger::model::EstimateValue::MapLocation { record_id, .. } => {
                    Some(record_id)
                }
                _ => None,
            })
            && let Some(record) = entry.records.iter().find(|r| r.id == record_id)
        {
            return record.observation.name.clone();
        }
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
    fonts.font_data.insert(
        "Atlas Han Supplement".into(),
        egui::FontData::from_static(include_bytes!("../assets/AtlasHanSupplement.ttf")).into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .extend(["Atlas CJK".into(), "Atlas Han Supplement".into()]);
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests;
