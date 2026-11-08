//! TT milliseconds relative to J2000, serialized as strings to preserve all bits.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Milliseconds in a Julian year of exactly 365.25 uniform days.
pub const JULIAN_YEAR_MS: i128 = 31_557_600_000;

/// Continuous TT milliseconds since 2000-01-01 12:00:00 TT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EpochTime(i128);

impl EpochTime {
    /// Construct from a signed millisecond offset.
    pub const fn from_ticks(ticks: i128) -> Self {
        Self(ticks)
    }

    /// Exact millisecond offset; callers must not downcast through floating point.
    pub const fn ticks(self) -> i128 {
        self.0
    }

    /// Shift by a checked millisecond duration.
    pub fn checked_add(self, milliseconds: i128) -> Result<Self, TimeError> {
        self.0
            .checked_add(milliseconds)
            .map(Self)
            .ok_or(TimeError::Overflow)
    }

    /// Age in integer Julian years before a caller-supplied reference epoch.
    ///
    /// BP datasets must supply their own reference (commonly 1950), not J2000.
    pub fn years_before(reference: Self, years: i128) -> Result<Self, TimeError> {
        let duration = years
            .checked_mul(JULIAN_YEAR_MS)
            .ok_or(TimeError::Overflow)?;
        reference
            .0
            .checked_sub(duration)
            .map(Self)
            .ok_or(TimeError::Overflow)
    }
}

impl TryFrom<String> for EpochTime {
    type Error = std::num::ParseIntError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse().map(Self)
    }
}

impl From<EpochTime> for String {
    fn from(value: EpochTime) -> Self {
        value.0.to_string()
    }
}

/// Precision stated by the source, separate from interval length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimePrecision {
    /// Billion years.
    Ga,
    /// Million years.
    Ma,
    /// Thousand years.
    Ka,
    /// Decade.
    Decade,
    /// Calendar year.
    Year,
    /// Calendar month.
    Month,
    /// Calendar day.
    Day,
    /// Second.
    Second,
    /// Millisecond.
    Millisecond,
}

/// A bounded estimate; uncertainty never silently collapses to its nominal value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstantEstimate {
    /// Inclusive earliest supported time.
    pub earliest: EpochTime,
    /// Chosen representative time.
    pub nominal: EpochTime,
    /// Inclusive latest supported time.
    pub latest: EpochTime,
    /// Source precision.
    pub precision: TimePrecision,
}

impl InstantEstimate {
    /// Reject inverted or out-of-range estimates, including imported JSON.
    pub fn validate(&self) -> Result<(), TimeError> {
        if self.earliest <= self.nominal && self.nominal <= self.latest {
            Ok(())
        } else {
            Err(TimeError::UnorderedEstimate)
        }
    }
}

/// Temporal interval with optional unknown endpoints, using an exclusive end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalSpan {
    /// First possible period of validity; `None` means unknown.
    pub start: Option<InstantEstimate>,
    /// End of validity; `None` means unknown, not proof of indefinite existence.
    pub end: Option<InstantEstimate>,
}

impl TemporalSpan {
    /// Check endpoints and require the nominal end to follow the nominal start.
    pub fn validate(&self) -> Result<(), TimeError> {
        for estimate in self.start.iter().chain(self.end.iter()) {
            estimate.validate()?;
        }
        if let (Some(start), Some(end)) = (self.start, self.end)
            && start.nominal >= end.nominal
        {
            return Err(TimeError::EmptySpan);
        }
        Ok(())
    }
}

/// Time conversion or validation failure.
#[derive(Debug, Error)]
pub enum TimeError {
    /// A duration cannot fit the continuous axis.
    #[error("time arithmetic exceeds the i128 millisecond axis")]
    Overflow,
    /// The nominal time lies outside the stated bounds.
    #[error("time estimate must satisfy earliest <= nominal <= latest")]
    UnorderedEstimate,
    /// Interval direction is inconsistent.
    #[error("exclusive interval end must follow its start")]
    EmptySpan,
}

#[cfg(test)]
mod tests;
