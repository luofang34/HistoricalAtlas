//! Exact calendar, geological-age, and millisecond time controls.

use std::collections::BTreeMap;

use atlas_core::{
    assertion::TemporalValue,
    calendar::{Calendar, CivilDate},
    identity::EntityId,
    time::EpochTime,
};
use atlas_ledger::timeline::{TimeSelection, Timeline};

use crate::{catalog::Atlas, error::AppError};

/// How the time entry is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeMode {
    /// January 1 in the proleptic Gregorian calendar, TT.
    Year,
    /// Integer million Julian years before J2000 TT.
    MillionYears,
    /// Exact signed TT milliseconds from J2000.
    Ticks,
}

/// A time input that never passes integer ticks through floating point.
pub struct TimeControl {
    /// Input convention.
    pub mode: TimeMode,
    /// User-editable decimal integer.
    pub value: String,
    /// Treatment of endpoint uncertainty.
    pub selection: TimeSelection,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            mode: TimeMode::Year,
            value: "1982".into(),
            selection: TimeSelection::Nominal,
        }
    }
}

impl TimeControl {
    /// Convert the input into the common exact axis.
    pub fn epoch(&self) -> Result<EpochTime, AppError> {
        Ok(match self.mode {
            TimeMode::Year => CivilDate {
                year: self.value.parse()?,
                month: 1,
                day: 1,
            }
            .to_epoch(Calendar::Gregorian, 0)?,
            TimeMode::MillionYears => EpochTime::years_before(
                EpochTime::from_ticks(0),
                self.value
                    .parse::<i128>()?
                    .checked_mul(1_000_000)
                    .ok_or(atlas_core::time::TimeError::Overflow)?,
            )?,
            TimeMode::Ticks => EpochTime::try_from(self.value.clone())?,
        })
    }
}

/// Selected temporal evidence serialized for an inspectable UI snapshot.
pub struct SelectedTime {
    /// Exact query epoch.
    pub at: EpochTime,
    /// Applied endpoint interpretation, independent of unsubmitted controls.
    pub selection: TimeSelection,
    /// Per-entity selected assertions and formation constraints.
    pub evidence: BTreeMap<EntityId, serde_json::Value>,
    /// Date-selected names, including competing values.
    pub names: BTreeMap<EntityId, Vec<String>>,
}

impl SelectedTime {
    /// Query the same timeline used by the command-line compiler.
    pub fn query(atlas: &Atlas, control: &TimeControl) -> Result<Self, AppError> {
        let at = control.epoch()?;
        let timeline = Timeline::new(&atlas.sources, &atlas.data)?;
        let selected = timeline.at(at, control.selection)?;
        let mut evidence = BTreeMap::new();
        let mut names = BTreeMap::new();
        for entity in selected.entities {
            let mut labels: Vec<_> = entity
                .properties
                .iter()
                .flat_map(|property| &property.candidates)
                .filter_map(|candidate| match &candidate.assertion.value {
                    TemporalValue::Name(name) => Some(name.text.clone()),
                    _ => None,
                })
                .collect();
            labels.sort();
            labels.dedup();
            names.insert(entity.entity_id, labels);
            evidence.insert(entity.entity_id, serde_json::to_value(entity)?);
        }
        Ok(Self {
            at,
            selection: control.selection,
            evidence,
            names,
        })
    }
}

#[cfg(test)]
mod tests;
