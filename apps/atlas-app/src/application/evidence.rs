use super::Application;
use atlas_core::{identity::EntityId, time::EpochTime};
use atlas_ledger::model::Record;
use egui::collapsing_header::CollapsingState;

impl Application {
    pub(super) fn evidence(&mut self, ui: &mut egui::Ui) {
        ui.heading("Evidence ledger");
        let Some(id) = self.selected else {
            ui.label("Select a place on the map or search by name.");
            return;
        };
        let mut navigate = None;
        let mut date_target = None;
        egui::ScrollArea::vertical()
            .id_salt("evidence-scroll")
            .show(ui, |ui| {
                ui.heading(self.label(id));
                ui.monospace(id.0.to_string());
                ui.separator();
                self.overlay_details(ui, id);
                navigate = self.hierarchy(ui, id);
                if let Some(entry) = self.atlas.entry(id) {
                    mapped_boundaries(ui, &entry.boundaries);
                    derived_boundaries(ui, &entry.derived_boundaries);
                    if self.layer == crate::scene::MapLayer::MappedPlaces {
                        mapped_locations(ui, &entry.estimates);
                        ui.separator();
                    }
                    ui.strong("Source observations");
                    for record in &entry.records {
                        if let Some(at) =
                            ui.push_id(record.id.0, |ui| self.record(ui, record)).inner
                        {
                            date_target = Some(at);
                        }
                    }
                    ui.separator();
                    ui.strong("At selected time");
                    if let Some(evidence) = self.selected_time.evidence.get(&id) {
                        json_value(ui, "Selected assertions", evidence);
                    } else {
                        ui.label("No dated assertion matches this selection.");
                    }
                    for estimate in &entry.estimates {
                        let id = ui.make_persistent_id(("estimate", estimate.id.0));
                        CollapsingState::load_with_default_open(ui.ctx(), id, false)
                            .show_header(ui, |ui| ui.label(&estimate.brief))
                            .body(|ui| json_value(ui, "Interpretation", estimate));
                    }
                    for dispute in &entry.disputes {
                        json_value(ui, "Source disagreement", dispute);
                    }
                }
            });
        if let Some(id) = navigate {
            self.select(id, true);
        }
        if let Some(at) = date_target {
            self.time.mode = crate::selection::TimeMode::Ticks;
            self.time.value = at.ticks().to_string();
            self.apply_time();
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

    fn record(&self, ui: &mut egui::Ui, record: &Record) -> Option<EpochTime> {
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
            coordinate_evidence(ui, record);
            if let Some(extent) = &record.observation.extent {
                ui.label(format!("Extent: {extent}"));
            }
            if let Some(population) = record.observation.population {
                ui.label(format!("Population: {population}"));
            }
            if let Some(households) = record.observation.households {
                ui.label(format!("Households: {households}"));
            }
            let date_target = record
                .source_date
                .as_ref()
                .and_then(|date| self.source_date(ui, date));
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
            date_target
        })
        .inner
    }
}

fn mapped_boundaries(
    ui: &mut egui::Ui,
    boundaries: &[atlas_boundary::geographic::GeographicBoundary],
) {
    for boundary in boundaries {
        ui.strong("Registered map boundary");
        let method = match boundary.boundary_method {
            atlas_boundary::sheet::BoundaryMethod::Surveyed => "Surveyed",
            atlas_boundary::sheet::BoundaryMethod::CompiledFromPlaces => {
                "Compiled from place locations"
            }
            atlas_boundary::sheet::BoundaryMethod::Unstated => "Unstated",
        };
        ui.label(format!("Source method: {method}"));
        ui.label(format!("Datum: {:?}", boundary.datum));
        if let Some(allowance) = boundary.registration_allowance_m {
            ui.label(format!(
                "Registration allowance: {:.1} km",
                allowance / 1000.0
            ));
        }
        ui.label("Effective dates: unknown");
        ui.label(format!("Registration: {}", boundary.registration));
        ui.separator();
    }
}

fn derived_boundaries(ui: &mut egui::Ui, boundaries: &[atlas_boundary::terrain::DerivedBoundary]) {
    for boundary in boundaries {
        ui.strong("Reconstructed boundary");
        ui.label("Terrain-cost partition from source locations and administrative membership");
        ui.label(format!(
            "Grid: {:.0} projected metres · elevation cost: {}",
            boundary.cell_m, boundary.slope_weight
        ));
        ui.label(format!(
            "Datum: {:?} · Effective dates: unknown",
            boundary.datum
        ));
        for comparison in &boundary.comparisons {
            ui.label(format!(
                "Area: {:.2} km² modeled · {:.2} km² reported",
                comparison.modeled_area_square_km, comparison.reported_area_square_km
            ));
            if let Some(tolerance) = comparison.fitting_tolerance_square_km {
                ui.label(format!(
                    "Fitted to the reported area · tolerance: {tolerance:.2} km²"
                ));
            }
            ui.collapsing("Neighbor checks", |ui| {
                for neighbor in &comparison.neighbors {
                    let status = match neighbor.shares_edge {
                        Some(true) => "shared edge",
                        Some(false) => "no shared edge",
                        None => "boundary not mapped",
                    };
                    ui.label(format!("{}: {status}", neighbor.name));
                    if let Some(review) = &neighbor.unresolved {
                        ui.label(format!("Unresolved source adjacency: {}", review.reason));
                    }
                }
            });
        }
        json_value(ui, "Reconstruction evidence", boundary);
        ui.separator();
    }
}

fn mapped_locations(ui: &mut egui::Ui, estimates: &[atlas_ledger::model::Estimate]) {
    for estimate in estimates {
        if let atlas_ledger::model::EstimateValue::MapLocation {
            point,
            datum,
            uncertainty_m,
            ..
        } = estimate.value
        {
            ui.strong("Map-derived location");
            ui.label(format!(
                "{:.5}°, {:.5}° · Datum: {datum:?}",
                point.longitude, point.latitude
            ));
            ui.label(format!(
                "Working positional allowance: {:.1} km",
                uncertainty_m / 1000.0
            ));
            ui.collapsing("Registration details", |ui| ui.label(&estimate.brief));
        }
    }
}

fn coordinate_evidence(ui: &mut egui::Ui, record: &Record) {
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
    if let Some(coordinate) = &record.observation.unresolved_coordinate {
        ui.label(format!(
            "{}  {}",
            coordinate.longitude_text, coordinate.latitude_text
        ));
        ui.label(format!(
            "Datum: {:?} · Resolution: {}′",
            coordinate.datum, coordinate.resolution_arcminutes
        ));
        ui.label(&coordinate.reason);
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
