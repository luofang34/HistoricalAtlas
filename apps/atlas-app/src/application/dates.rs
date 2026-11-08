use atlas_core::{
    calendar::{Calendar, CivilDate, SourceDate},
    time::EpochTime,
};

use super::Application;

impl Application {
    pub(super) fn source_date(&self, ui: &mut egui::Ui, date: &SourceDate) -> Option<EpochTime> {
        ui.label(format!("Source date: {}", date.text));
        let conversion = date.era_conversion.as_ref()?;
        let calendar = match conversion.calendar {
            Calendar::Gregorian => "Gregorian",
            Calendar::Julian => "Julian",
            Calendar::Reform1582 => "Julian / Gregorian (1582 reform)",
        };
        ui.label(format!(
            "{} to {} · {calendar}",
            civil_date(conversion.first_day),
            civil_date(conversion.last_day)
        ));
        if let Some(source) = self
            .atlas
            .sources
            .sources
            .iter()
            .find(|source| source.id == conversion.source_id)
            && let Some(url) = &source.url
        {
            ui.hyperlink_to(format!("Calendar: {}", source.title), url);
        }
        match conversion.to_estimate(0) {
            Ok(instant) => ui
                .button("Show date on timeline")
                .on_hover_text("Select the midpoint of the cited period on the TT calendar axis.")
                .clicked()
                .then_some(instant.nominal),
            Err(error) => {
                ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
                None
            }
        }
    }
}

fn civil_date(date: CivilDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}
