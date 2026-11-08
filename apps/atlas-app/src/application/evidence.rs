use super::Application;
use atlas_core::identity::EntityId;
use atlas_ledger::model::Record;

impl Application {
    pub(super) fn evidence(&mut self, ui: &mut egui::Ui) {
        ui.heading("Evidence ledger");
        let Some(id) = self.selected else {
            ui.label("Select a place on the map or search by name.");
            return;
        };
        let mut navigate = None;
        egui::ScrollArea::vertical()
            .id_salt("evidence-scroll")
            .show(ui, |ui| {
                ui.heading(self.label(id));
                ui.monospace(id.0.to_string());
                ui.separator();
                navigate = self.hierarchy(ui, id);
                if let Some(entry) = self.atlas.entry(id) {
                    ui.strong("Source observations");
                    for record in &entry.records {
                        ui.push_id(record.id.0, |ui| self.record(ui, record));
                    }
                    ui.separator();
                    ui.strong("At selected time");
                    if let Some(evidence) = self.selected_time.evidence.get(&id) {
                        json_value(ui, "Selected assertions", evidence);
                    } else {
                        ui.label("No dated assertion matches this selection.");
                    }
                    for estimate in &entry.estimates {
                        ui.collapsing(&estimate.brief, |ui| {
                            json_value(ui, "Interpretation", estimate)
                        });
                    }
                    for dispute in &entry.disputes {
                        json_value(ui, "Source disagreement", dispute);
                    }
                }
            });
        if let Some(id) = navigate {
            self.select(id, true);
        }
    }

    fn hierarchy(&self, ui: &mut egui::Ui, id: EntityId) -> Option<EntityId> {
        let mut target = None;
        ui.collapsing("Source hierarchy", |ui| {
            for parent in self.atlas.parents(id) {
                if ui
                    .button(format!("Parent: {}", self.atlas.label(parent)))
                    .clicked()
                {
                    target = Some(parent);
                }
            }
            for child in self.atlas.children(id) {
                if ui.button(self.atlas.label(child)).clicked() {
                    target = Some(child);
                }
            }
        });
        target
    }

    fn record(&self, ui: &mut egui::Ui, record: &Record) {
        ui.group(|ui| {
            if let Some(source) = self
                .atlas
                .sources
                .sources
                .iter()
                .find(|source| source.id == record.source_id)
            {
                if let Some(url) = &source.url {
                    ui.hyperlink_to(&source.title, url);
                } else {
                    ui.strong(&source.title);
                }
            }
            ui.label(format!(
                "Printed page: {} · Scan page: {}",
                record.page_or_sheet.as_deref().unwrap_or("—"),
                record
                    .scan_page
                    .map_or_else(|| "—".into(), |page| page.to_string())
            ));
            ui.label(&record.locator);
            ui.label(&record.observation.name);
            if let Some(pinyin) = &record.observation.pinyin {
                ui.label(pinyin);
            }
            if let Some(coordinate) = &record.observation.coordinate {
                ui.label(format!(
                    "{}  {}",
                    coordinate.longitude_text, coordinate.latitude_text
                ));
                ui.label(format!(
                    "Datum: {:?} · Resolution: {}′",
                    coordinate.datum, coordinate.resolution_arcminutes
                ));
            }
            if let Some(population) = record.observation.population {
                ui.label(format!("Population: {population}"));
            }
            if let Some(households) = record.observation.households {
                ui.label(format!("Households: {households}"));
            }
            if let Some(date) = &record.source_date {
                ui.label(format!("Source date: {}", date.text));
            }
            if let Some(quote) = &record.verbatim_quote {
                ui.label(quote);
            }
            if let Some(note) = &record.observation.note {
                ui.label(note);
            }
            for name in &record.observation.former_names {
                ui.label(format!("Former name: {}", name.name));
            }
            json_value(ui, "Complete record", record);
        });
    }
}

fn json_value(ui: &mut egui::Ui, title: &str, value: &impl serde::Serialize) {
    ui.collapsing(title, |ui| match serde_json::to_string_pretty(value) {
        Ok(json) => {
            ui.add(egui::Label::new(egui::RichText::new(json).monospace()).wrap());
        }
        Err(error) => {
            ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
        }
    });
}
