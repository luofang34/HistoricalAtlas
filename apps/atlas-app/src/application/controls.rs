use super::Application;
use crate::{navigation::Camera, scene::MapLayer, selection::TimeMode};
use atlas_geo::body::BodyId;
use atlas_ledger::timeline::TimeSelection;

impl Application {
    pub(super) fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Historical Atlas");
            for body in [BodyId::Earth, BodyId::Moon, BodyId::Mars] {
                if ui
                    .selectable_value(&mut self.body, body, format!("{body:?}"))
                    .changed()
                {
                    self.camera = Camera::for_body(body);
                    self.selected = None;
                    self.hits.clear();
                    self.dirty = true;
                }
            }
        });
        self.time_controls(ui);
        self.shortcuts(ui);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }

    fn time_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("time-mode")
                .selected_text(format!("{:?}", self.time.mode))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.time.mode, TimeMode::Year, "Gregorian year (TT)");
                    ui.selectable_value(
                        &mut self.time.mode,
                        TimeMode::MillionYears,
                        "Ma before J2000",
                    );
                    ui.selectable_value(
                        &mut self.time.mode,
                        TimeMode::Ticks,
                        "TT milliseconds from J2000",
                    );
                });
            ui.add(egui::TextEdit::singleline(&mut self.time.value).desired_width(180.0));
            egui::ComboBox::from_id_salt("time-selection")
                .selected_text(format!("{:?}", self.time.selection))
                .show_ui(ui, |ui| {
                    for selection in [
                        TimeSelection::Nominal,
                        TimeSelection::Possible,
                        TimeSelection::Certain,
                    ] {
                        ui.selectable_value(
                            &mut self.time.selection,
                            selection,
                            format!("{selection:?}"),
                        );
                    }
                });
            if ui.button("Apply time").clicked() {
                self.apply_time();
            }
            ui.label(format!("Applied: {} TT ms", self.selected_time.at.ticks()));
        });
    }

    fn shortcuts(&mut self, ui: &mut egui::Ui) {
        let previous = self.layer;
        ui.horizontal_wrapped(|ui| {
            let mut year = self
                .time
                .value
                .parse::<i32>()
                .unwrap_or(1982)
                .clamp(1950, 1984);
            if ui
                .add(egui::Slider::new(&mut year, 1950..=1984).text("Year"))
                .changed()
            {
                self.time.mode = TimeMode::Year;
                self.time.value = year.to_string();
                self.apply_time();
            }
            for (label, mode, value) in [
                ("669 CE", TimeMode::Year, "669"),
                ("1381 CE", TimeMode::Year, "1381"),
                ("240 Ma", TimeMode::MillionYears, "240"),
                ("4510 Ma", TimeMode::MillionYears, "4510"),
            ] {
                if ui.button(label).clicked() {
                    self.time.mode = mode;
                    self.time.value = value.into();
                    self.apply_time();
                }
            }
            self.dirty |= ui
                .selectable_value(
                    &mut self.layer,
                    MapLayer::MappedPlaces,
                    "Map-derived locations",
                )
                .changed();
            self.dirty |= ui
                .selectable_value(
                    &mut self.layer,
                    MapLayer::Observations,
                    "Source observations",
                )
                .changed();
            self.dirty |= ui
                .selectable_value(
                    &mut self.layer,
                    MapLayer::SelectedTime,
                    "Evidence at selected time",
                )
                .changed();
            self.dirty |= ui
                .checkbox(&mut self.territory_visible, "Territory nodes")
                .on_hover_text("Administrative and seat links for the selected place.")
                .changed();
        });
        if self.layer != previous {
            self.sheet.visible = false;
        }
    }

    pub(super) fn search_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Places");
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Name, pinyin, former name, UUID"),
        );
        let ids = self.atlas.search(&self.search, self.body);
        ui.label(format!("{} entities", ids.len()));
        egui::ScrollArea::vertical()
            .id_salt("search-results")
            .show(ui, |ui| {
                if !self.hits.is_empty() {
                    ui.strong("At this map point");
                    for id in self.hits.clone() {
                        if ui
                            .selectable_label(self.selected == Some(id), self.label(id))
                            .clicked()
                        {
                            self.select(id, false);
                        }
                    }
                    ui.separator();
                }
                for id in ids {
                    let response = ui.selectable_label(self.selected == Some(id), self.label(id));
                    if response.clicked() {
                        self.select(id, true);
                    }
                    response
                        .on_hover_text(self.atlas.entry(id).map_or("", |entry| &entry.entity.key));
                }
            });
    }
}
