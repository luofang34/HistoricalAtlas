use super::Application;
use crate::{
    navigation::Camera,
    overlay::{Overlay, OverlayInput, validate_link},
    selection::TimeMode,
};
use atlas_core::identity::EntityId;
use atlas_ledger::timeline::TimeSelection;

#[derive(Default)]
pub(super) struct OverlayState {
    pub(super) source: Option<Overlay>,
    pub(super) visible: bool,
    selected: Option<String>,
    error: Option<String>,
}

impl Application {
    /// Apply a completed external request, keeping the atlas usable on failure.
    pub fn load_overlay(&mut self, input: OverlayInput) {
        let result = input.content.and_then(|json| {
            Overlay::parse(&self.atlas, input.origin.clone(), &json)
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(overlay) => {
                self.overlay.source = Some(overlay);
                self.overlay.selected = None;
                self.overlay.visible = true;
                self.overlay.error = None;
                self.dirty = true;
            }
            Err(error) => self.overlay.error = Some(format!("{}: {error}", input.origin)),
        }
    }

    pub(super) fn overlay_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = &self.overlay.error {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Event layer: {error}"));
        }
        let Some(overlay) = &self.overlay.source else {
            return;
        };
        ui.heading("External events");
        ui.label(&overlay.package().title);
        self.dirty |= ui
            .checkbox(&mut self.overlay.visible, "Show event markers")
            .changed();
        let mut selected = None;
        egui::ScrollArea::vertical()
            .id_salt("external-events")
            .max_height(220.0)
            .show(ui, |ui| {
                for event in &overlay.package().events {
                    let response = ui.selectable_label(
                        self.overlay.selected.as_deref() == Some(&event.id),
                        &event.title,
                    );
                    if response.clicked() {
                        selected = Some(event.id.clone());
                    }
                    let status = match event
                        .matches(self.selected_time.at, self.selected_time.selection)
                    {
                        Ok(true) => "At selected time",
                        Ok(false)
                            if event.validity.start.is_none() && event.validity.end.is_none() =>
                        {
                            "Undated"
                        }
                        Ok(false) => "Outside selected time",
                        Err(_) => "Invalid date",
                    };
                    response.on_hover_text(status);
                }
            });
        ui.separator();
        if let Some(id) = selected {
            self.focus_event(&id);
        }
    }

    fn focus_event(&mut self, id: &str) {
        let Some(event) = self
            .overlay
            .source
            .as_ref()
            .and_then(|overlay| overlay.package().events.iter().find(|event| event.id == id))
        else {
            return;
        };
        let target = event
            .anchors(&self.atlas, self.body)
            .first()
            .copied()
            .or_else(|| {
                event
                    .entities
                    .first()
                    .and_then(|id| self.atlas.resolve(*id))
            });
        let at = match event.focus_time() {
            Ok(at) => at,
            Err(error) => {
                self.overlay.error = Some(error.to_string());
                return;
            }
        };
        self.overlay.selected = Some(id.to_owned());
        self.overlay.visible = true;
        if let Some(target) = target {
            if let Some(body) = self.atlas.entry(target).map(|entry| entry.entity.body)
                && self.body != body
            {
                self.body = body;
                self.camera = Camera::for_body(body);
                self.camera_pending = true;
                self.layer = crate::scene::MapLayer::Observations;
                self.hits.clear();
            }
            self.select(target, true);
        }
        if let Some(at) = at {
            self.time.mode = TimeMode::Ticks;
            self.time.value = at.ticks().to_string();
            self.time.selection = TimeSelection::Nominal;
            self.apply_time();
        }
    }

    pub(super) fn overlay_details(&self, ui: &mut egui::Ui, id: EntityId) {
        let Some(overlay) = &self.overlay.source else {
            return;
        };
        for event in &overlay.package().events {
            if !event.anchors(&self.atlas, self.body).contains(&id) {
                continue;
            }
            let active = event.matches(self.selected_time.at, self.selected_time.selection);
            if self.overlay.selected.as_deref() != Some(&event.id) && !matches!(active, Ok(true)) {
                continue;
            }
            ui.group(|ui| {
                ui.strong("External event");
                ui.heading(&event.title);
                ui.label(format!("Package: {}", overlay.package().title));
                if validate_link(overlay.origin()).is_ok() {
                    ui.hyperlink_to("Open event package", overlay.origin());
                } else {
                    ui.label(overlay.origin());
                }
                if let Some(description) = &event.description {
                    ui.label(description);
                }
                if let Some(url) = &event.url {
                    ui.hyperlink_to("Event source", url);
                }
                if event.validity.start.is_none() && event.validity.end.is_none() {
                    ui.label("Undated event");
                }
            });
        }
    }
}
