//! Dating an entity's formation with bounded, one-sided, or approximate evidence.

use serde::{Deserialize, Serialize};

use crate::time::{EpochTime, InstantEstimate, TimeError, TimeMembership, TimePrecision};

/// Temporal evidence for the formation or establishment of an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "constraint", rename_all = "snake_case", deny_unknown_fields)]
pub enum OriginTime {
    /// Formation dated within a stated range.
    Estimated {
        /// Formation time and its uncertainty.
        instant: InstantEstimate,
    },
    /// Evidence that the entity had formed by a dated observation or process.
    NoLaterThan {
        /// Dating of the limiting evidence; earlier formation remains possible.
        limit: InstantEstimate,
    },
    /// Representative age with no quantified uncertainty in the source.
    Approximate {
        /// Representative time used for nominal display.
        nominal: EpochTime,
        /// Source precision.
        precision: TimePrecision,
    },
}

impl OriginTime {
    /// Check the ordering of every supplied uncertainty range.
    pub fn validate(&self) -> Result<(), TimeError> {
        match self {
            Self::Estimated { instant } => instant.validate(),
            Self::NoLaterThan { limit } => limit.validate(),
            Self::Approximate { .. } => Ok(()),
        }
    }

    /// Representative formation or limiting time for nominal display.
    pub fn nominal(&self) -> EpochTime {
        match self {
            Self::Estimated { instant } => instant.nominal,
            Self::NoLaterThan { limit } => limit.nominal,
            Self::Approximate { nominal, .. } => *nominal,
        }
    }

    /// Support that formation had occurred by `at`, independent of later survival.
    pub fn occurred_by(&self, at: EpochTime) -> Result<TimeMembership, TimeError> {
        self.validate()?;
        let latest = match self {
            Self::Estimated { instant } if at < instant.earliest => {
                return Ok(TimeMembership::Outside);
            }
            Self::Estimated { instant } => Some(instant.latest),
            Self::NoLaterThan { limit } => Some(limit.latest),
            Self::Approximate { .. } => None,
        };
        Ok(if latest.is_some_and(|latest| at >= latest) {
            TimeMembership::Certain
        } else {
            TimeMembership::Possible
        })
    }
}

#[cfg(test)]
mod tests;
