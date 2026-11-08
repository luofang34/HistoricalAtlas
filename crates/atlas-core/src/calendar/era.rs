//! Convert reviewed era-calendar periods to bounded instants.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Calendar, CalendarError, CivilDate};
use crate::time::{InstantEstimate, TimeError, TimePrecision};

const DAY_MS: i128 = 86_400_000;

/// A period within a historical lunisolar era year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "precision", rename_all = "snake_case", deny_unknown_fields)]
pub enum EraPeriod {
    /// The whole cited era year, including any intercalary month.
    Year,
    /// A numbered lunar month.
    Month {
        /// Month number, from one through twelve.
        month: u8,
        /// Whether the citation specifies the intercalary occurrence.
        leap: bool,
    },
    /// A day within a numbered lunar month.
    Day {
        /// Month number, from one through twelve.
        month: u8,
        /// Whether the citation specifies the intercalary occurrence.
        leap: bool,
        /// Day number, from one through thirty.
        day: u8,
    },
}

impl EraPeriod {
    fn limits(self) -> Result<(TimePrecision, Option<i128>), EraError> {
        match self {
            Self::Year => Ok((TimePrecision::Year, None)),
            Self::Month { month: 1..=12, .. } => Ok((TimePrecision::Month, Some(30))),
            Self::Day {
                month: 1..=12,
                day: 1..=30,
                ..
            } => Ok((TimePrecision::Day, Some(1))),
            _ => Err(EraError::InvalidPeriod(self)),
        }
    }
}

/// A reviewed correspondence from an era period to inclusive civil dates.
///
/// The cited table resolves era names, intercalation, and changes to the year start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EraConcordance {
    /// Dynasty as identified by the calendar source.
    pub dynasty: String,
    /// Era name as identified by the calendar source.
    pub era: String,
    /// Positive year counted within that era.
    pub year: u32,
    /// Precision and lunar month/day, where stated.
    pub period: EraPeriod,
    /// Calendar used for the civil-date columns of the cited table.
    pub calendar: Calendar,
    /// First included civil day.
    pub first_day: CivilDate,
    /// Last included civil day.
    pub last_day: CivilDate,
    /// Catalogued online calendar source.
    pub source_id: String,
    /// Reproducible query or location in that source.
    pub locator: String,
}

impl EraConcordance {
    /// Convert the whole period, choosing its midpoint as the nominal instant.
    ///
    /// A zero offset expresses calendar dates on the TT axis. A physical local
    /// midnight needs a separately established TT-minus-civil offset.
    pub fn to_estimate(&self, tt_minus_civil_ms: i128) -> Result<InstantEstimate, EraError> {
        if self.dynasty.trim().is_empty() || self.era.trim().is_empty() || self.year == 0 {
            return Err(EraError::InvalidEra {
                dynasty: self.dynasty.clone(),
                era: self.era.clone(),
                year: self.year,
            });
        }
        if self.source_id.trim().is_empty() || self.locator.trim().is_empty() {
            return Err(EraError::MissingCitation);
        }
        let (precision, maximum_days) = self.period.limits()?;
        let first = self.first_day.to_epoch(self.calendar, 0)?;
        let last = self.last_day.to_epoch(self.calendar, 0)?;
        let duration = last.ticks() - first.ticks() + DAY_MS;
        if duration <= 0 || maximum_days.is_some_and(|days| duration > days * DAY_MS) {
            return Err(EraError::InvalidBounds {
                first: self.first_day,
                last: self.last_day,
                period: self.period,
            });
        }
        let earliest = first.checked_add(tt_minus_civil_ms)?;
        Ok(InstantEstimate {
            earliest,
            nominal: earliest.checked_add((duration - 1) / 2)?,
            latest: earliest.checked_add(duration - 1)?,
            precision,
        })
    }
}

/// Invalid era notation, table bounds, or time-axis conversion.
#[derive(Debug, Error)]
pub enum EraError {
    /// Dynasty, era, or positive era year is missing.
    #[error("invalid era date {dynasty} {era} year {year}")]
    InvalidEra {
        /// Supplied dynasty.
        dynasty: String,
        /// Supplied era.
        era: String,
        /// Supplied era year.
        year: u32,
    },
    /// Lunar month or day is outside its numeric domain.
    #[error("invalid lunar period {0:?}")]
    InvalidPeriod(EraPeriod),
    /// The table interval is inverted or exceeds its stated precision.
    #[error("civil bounds {first:?} through {last:?} are invalid for {period:?}")]
    InvalidBounds {
        /// Supplied first day.
        first: CivilDate,
        /// Supplied last day.
        last: CivilDate,
        /// Supplied precision.
        period: EraPeriod,
    },
    /// A conversion needs an identifiable table and query.
    #[error("era concordance requires a source and locator")]
    MissingCitation,
    /// A civil date fails calendar validation.
    #[error(transparent)]
    Calendar(#[from] CalendarError),
    /// Applying the offset exceeds the time axis.
    #[error(transparent)]
    Time(#[from] TimeError),
}

#[cfg(test)]
mod tests;
