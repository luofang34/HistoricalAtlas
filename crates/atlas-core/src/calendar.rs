//! Civil calendar conversion and cited historical era concordances.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::time::{EpochTime, TimeError};

pub mod era;

/// Calendar used to interpret a numeric civil date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Calendar {
    /// Gregorian calendar extended to all astronomical years.
    Gregorian,
    /// Julian calendar extended to all astronomical years.
    Julian,
    /// Julian through 1582-10-04, Gregorian from 1582-10-15.
    Reform1582,
}

/// A source date preserved independently of its interpreted time interval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDate {
    /// Exact expression, such as 康熙二十六年四月 or 240 Ma BP.
    pub text: String,
    /// Calendar or dating convention named by the editor.
    pub convention: String,
    /// Citation or explanation for conversion, absent when unconverted.
    pub conversion_basis: Option<String>,
    /// Reviewed era-to-civil concordance, including its calendar source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub era_conversion: Option<era::EraConcordance>,
}

/// Numeric civil date using astronomical year numbering (year zero is 1 BCE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CivilDate {
    /// Astronomical year.
    pub year: i64,
    /// Month, 1 through 12.
    pub month: u8,
    /// Day, subject to the chosen calendar.
    pub day: u8,
}

impl CivilDate {
    /// Convert local midnight to the TT axis with an explicit TT-minus-civil offset.
    ///
    /// UTC inputs require a dated leap-second/TT conversion. Historical local time
    /// needs longitude/timezone and delta-T evidence. Zero is appropriate for TT
    /// calendar coordinates, not a default assumption about an observed UTC time.
    pub fn to_epoch(
        self,
        calendar: Calendar,
        tt_minus_civil_ms: i128,
    ) -> Result<EpochTime, CalendarError> {
        let calendar = self.resolve_calendar(calendar)?;
        self.validate(calendar)?;
        let a = (14 - i128::from(self.month)).div_euclid(12);
        let y = i128::from(self.year) + 4800 - a;
        let m = i128::from(self.month) + 12 * a - 3;
        let base = i128::from(self.day) + (153 * m + 2).div_euclid(5) + 365 * y + y.div_euclid(4);
        let jdn = match calendar {
            Calendar::Gregorian => base - y.div_euclid(100) + y.div_euclid(400) - 32045,
            Calendar::Julian | Calendar::Reform1582 => base - 32083,
        };
        EpochTime::from_ticks((jdn - 2_451_545) * 86_400_000 - 43_200_000)
            .checked_add(tt_minus_civil_ms)
            .map_err(CalendarError::Time)
    }

    fn resolve_calendar(self, calendar: Calendar) -> Result<Calendar, CalendarError> {
        if calendar != Calendar::Reform1582 {
            return Ok(calendar);
        }
        let tuple = (self.year, self.month, self.day);
        if tuple <= (1582, 10, 4) {
            Ok(Calendar::Julian)
        } else if tuple >= (1582, 10, 15) {
            Ok(Calendar::Gregorian)
        } else {
            Err(CalendarError::ReformGap(self))
        }
    }

    fn validate(self, calendar: Calendar) -> Result<(), CalendarError> {
        let leap = self.year.rem_euclid(4) == 0
            && (calendar == Calendar::Julian
                || self.year.rem_euclid(100) != 0
                || self.year.rem_euclid(400) == 0);
        let days = match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => 0,
        };
        if self.day == 0 || self.day > days {
            Err(CalendarError::InvalidDate(self))
        } else {
            Ok(())
        }
    }
}

/// A calendar conversion failure retaining the input date.
#[derive(Debug, Error)]
pub enum CalendarError {
    /// Nonexistent day/month.
    #[error("invalid date {0:?}")]
    InvalidDate(CivilDate),
    /// A date removed by the 1582 reform.
    #[error("date {0:?} lies in the 1582 reform gap")]
    ReformGap(CivilDate),
    /// Offset overflows the time axis.
    #[error("calendar offset: {0}")]
    Time(#[source] TimeError),
}

#[cfg(test)]
mod tests;
