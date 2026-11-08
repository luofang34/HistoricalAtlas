use thiserror::Error;

use super::PlateId;
use crate::rotation::RotationError;

/// Invalid rotation input or unavailable reconstruction circuit.
#[derive(Debug, Error)]
pub enum PlateModelError {
    /// Missing PLATES4 columns.
    #[error("rotation line {line} requires six numerical columns")]
    Columns {
        /// One-based source line.
        line: usize,
    },
    /// Invalid plate identifier.
    #[error("rotation line {line}, {field}: {source}")]
    Identifier {
        /// One-based source line.
        line: usize,
        /// Moving or fixed plate column.
        field: &'static str,
        /// Number parser failure.
        #[source]
        source: std::num::ParseIntError,
    },
    /// Invalid numerical rotation field.
    #[error("rotation line {line}, {field}: {source}")]
    Number {
        /// One-based source line.
        line: usize,
        /// Age, latitude, longitude, or angle column.
        field: &'static str,
        /// Number parser failure.
        #[source]
        source: std::num::ParseFloatError,
    },
    /// Invalid sequence structure.
    #[error("plate {plate:?}: {reason}")]
    Sequence {
        /// Moving plate.
        plate: PlateId,
        /// Broken sequence invariant.
        reason: String,
    },
    /// Invalid source pole or angle.
    #[error("rotation line {line}: {source}")]
    Sample {
        /// One-based source line.
        line: usize,
        /// Geometry error.
        #[source]
        source: RotationError,
    },
    /// Invalid geological query age.
    #[error("reconstruction age must be finite and nonnegative, got {0} Ma")]
    Age(f64),
    /// A circuit edge is unavailable at the query age.
    #[error("plate {plate:?} has no rotation at {age_ma} Ma")]
    Missing {
        /// Unresolved plate.
        plate: PlateId,
        /// Query age.
        age_ma: f64,
    },
    /// Active fixed-plate relations form a cycle.
    #[error("rotation circuit cycles at plate {plate:?} at {age_ma} Ma")]
    Cycle {
        /// Repeated plate.
        plate: PlateId,
        /// Query age.
        age_ma: f64,
    },
    /// Quaternion or coordinate calculation failed.
    #[error("plate rotation: {0}")]
    Rotation(#[from] RotationError),
}
