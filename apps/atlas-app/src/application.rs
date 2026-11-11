//! Shared atlas interaction and evidence panels.

use crate::{
    catalog::Atlas,
    error::AppError,
    navigation::Camera,
    scene::{self, MapLayer, ScenePort},
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
mod streaming;

/// Application state independent of the windowing platform.
pub struct Application {
    #[cfg(target_arch = "wasm32")]
    streaming: streaming::StreamingLoads,
    atlas: Atlas,
    basemap: scene::basemap::Manifest,
    body: BodyId,
    time: TimeControl,
    selected_time: SelectedTime,
    selected: Option<EntityId>,
    search: String,
    /// Matches for the query and body they were computed for, kept until either changes.
    search_results: Option<(String, BodyId, Vec<EntityId>)>,
    hits: Vec<EntityId>,
    layer: MapLayer,
    territory_visible: bool,
    derived_visible: bool,
    crustal_pins: bool,
    camera: Camera,
    camera_pending: bool,
    map_size: [f64; 2],
    map: Option<Box<dyn ScenePort>>,
    dirty: bool,
    selection_dirty: bool,
    error: Option<String>,
    overlay: overlay::OverlayState,
    sheet: sheet::SheetView,
}

impl Application {
    /// Start with source-map geometry on the Earth globe and the 1982 time selection.
    pub fn new(atlas: Atlas) -> Result<Self, AppError> {
        let time = TimeControl::default();
        let selected_time = SelectedTime::query(&atlas, &time)?;
        Ok(Self {
            #[cfg(target_arch = "wasm32")]
            streaming: streaming::StreamingLoads::new(&atlas),
            atlas,
            basemap: scene::basemap::Manifest::bundled()?,
            body: BodyId::Earth,
            time,
            selected_time,
            selected: None,
            search: String::new(),
            search_results: None,
            hits: Vec::new(),
            layer: MapLayer::MappedPlaces,
            territory_visible: false,
            derived_visible: false,
            crustal_pins: false,
            camera: Camera::for_body(BodyId::Earth),
            camera_pending: true,
            map_size: [800.0, 600.0],
            map: None,
            dirty: true,
            selection_dirty: true,
            error: None,
            overlay: overlay::OverlayState::default(),
            sheet: sheet::SheetView::default(),
        })
    }

    /// Set the publication asset inventory before attaching a GPU.
    pub fn set_basemap(&mut self, manifest: scene::basemap::Manifest) {
        self.basemap = manifest;
    }

    /// Attach the GPU renderer after the window has a drawing surface.
    pub fn attach(&mut self, gpu: egui_wgpu::RenderState) -> Result<(), AppError> {
        self.map = Some(scene::create_scene(
            gpu,
            self.points()?,
            self.body,
            &self.basemap,
        )?);
        self.dirty = true;
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
        self.selection_dirty = true;
        self.error = None;
        if focus && !self.atlas.drawings(id).is_empty() && self.focus_point(id).is_none() {
            self.sheet = sheet::SheetView::open();
        }
        if focus && let Some(point) = self.focus_point(id) {
            self.camera_pending = true;
            self.camera.center = point;
            self.camera.zoom = if self.body == BodyId::Earth {
                12.0
            } else {
                5.0
            };
        }
        if focus
            && let Some(camera) = scene::boundaries::frame(
                &self.atlas,
                id,
                self.map_size,
                self.derived_visible,
                (self.layer == MapLayer::SelectedTime).then_some(&self.selected_time),
            )
        {
            self.camera = camera;
            self.camera_pending = true;
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
                .or_else(|| {
                    scene::boundaries::center(&self.atlas, id, self.derived_visible, None)
                });
        }
        if self.layer == MapLayer::SelectedTime {
            for record in &entry.records {
                if let Some(pos) = self.selected_time.paleo.get(&record.id).or_else(|| {
                    self.crustal_pins
                        .then(|| self.selected_time.crustal_pins.get(&record.id))
                        .flatten()
                }) {
                    return Some(pos.reconstructed);
                }
            }
            if self.selected_time.evidence.contains_key(&id) && !self.selected_time.is_excluded(id)
            {
                if let Some(point) = entry
                    .records
                    .iter()
                    .find_map(|record| record.observation.coordinate.as_ref().map(|c| c.point))
                {
                    return Some(point);
                }
                if let Some(point) =
                    entry
                        .estimates
                        .iter()
                        .find_map(|estimate| match estimate.value {
                            atlas_ledger::model::EstimateValue::MapLocation { point, .. } => {
                                Some(point)
                            }
                            _ => None,
                        })
                {
                    return Some(point);
                }
            }
            return scene::boundaries::center(
                &self.atlas,
                id,
                self.derived_visible,
                Some(&self.selected_time),
            );
        }
        entry
            .records
            .iter()
            .find_map(|record| record.observation.coordinate.as_ref().map(|c| c.point))
            .or_else(|| scene::boundaries::center(&self.atlas, id, self.derived_visible, None))
    }

    fn points(&self) -> Result<serde_json::Value, AppError> {
        let mut collection = scene::features_with_pins(
            &self.atlas,
            self.body,
            &self.selected_time,
            self.layer,
            None,
            self.crustal_pins,
        );
        if let Some(features) = collection
            .get_mut("features")
            .and_then(serde_json::Value::as_array_mut)
        {
            features.extend(scene::boundaries::features(
                &self.atlas,
                self.body,
                (self.layer == MapLayer::SelectedTime).then_some(&self.selected_time),
                None,
                self.derived_visible,
            )?);
        }
        if self.overlay.visible
            && let Some(overlay) = &self.overlay.source
            && let Some(features) = collection
                .get_mut("features")
                .and_then(serde_json::Value::as_array_mut)
        {
            features.extend(overlay.features(&self.atlas, self.body, &self.selected_time, None)?);
        }
        Ok(collection)
    }

    fn selection_features(&self) -> Result<Vec<serde_json::Value>, AppError> {
        match self.selected.filter(|_| self.territory_visible) {
            Some(id) => Ok(scene::territories::features(
                &self.atlas,
                id,
                &self.selected_time,
                self.layer,
            )?),
            None => Ok(Vec::new()),
        }
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

/// Install the publication font inventory alongside egui's default fonts.
#[cfg(not(target_arch = "wasm32"))]
pub fn configure_fonts_blocking(ctx: &egui::Context) -> Result<(), crate::fonts::FontError> {
    crate::fonts::configure_blocking(
        ctx,
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
    )
}

#[cfg(test)]
mod tests;
