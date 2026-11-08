//! GPlates rotation sequences and time-dependent plate circuits.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{coordinates::LonLat, rotation::EulerRotation};

mod circuit;
mod error;
mod parser;
mod sequence;
pub use error::PlateModelError;

/// Numeric reconstruction identity within one plate model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlateId(pub u32);

/// A total finite rotation at a geological age.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotationSample {
    /// Millions of years before the model's present.
    pub age_ma: f64,
    /// Rotation of the moving plate relative to its fixed plate.
    pub rotation: EulerRotation,
    /// One-based line in the input rotation file.
    pub source_line: usize,
    /// Source comment following the six numerical columns.
    pub description: String,
}

/// A contiguous history for a moving/fixed plate pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotationSequence {
    /// Plate whose motion is described.
    pub moving_plate: PlateId,
    /// Plate relative to which the rotations are expressed.
    pub fixed_plate: PlateId,
    /// Chronological samples, youngest first; equivalent duplicate ages are allowed.
    pub samples: Vec<RotationSample>,
}

/// A selected circuit edge, including the source interpolation bracket.
#[derive(Debug, Clone, Serialize)]
pub struct RotationStep {
    /// Moving plate.
    pub moving_plate: PlateId,
    /// Relative reference plate.
    pub fixed_plate: PlateId,
    /// Younger source sample line.
    pub younger_line: usize,
    /// Older source sample line; equal to the younger line at an exact sample.
    pub older_line: usize,
    /// Interpolated relative rotation.
    pub rotation: EulerRotation,
}

/// An anchored rotation and both circuits used to calculate it.
#[derive(Debug, Clone, Serialize)]
pub struct PlateRotation {
    /// Rotation from present-day coordinates to the selected reconstruction frame.
    pub rotation: EulerRotation,
    /// Moving-plate circuit, ending at plate zero.
    pub circuit: Vec<RotationStep>,
    /// Anchor-plate circuit, whose inverse defines the selected reference frame.
    pub anchor_circuit: Vec<RotationStep>,
}

/// Interface for interchangeable plate-motion models.
pub trait PlateMotion {
    /// Resolve an anchored total rotation at a geological age in Ma.
    fn rotation(
        &self,
        moving: PlateId,
        age_ma: f64,
        anchor: PlateId,
    ) -> Result<PlateRotation, PlateModelError>;
}

/// Validated finite rotations indexed by moving plate.
#[derive(Debug, Clone)]
pub struct RotationModel {
    sequences: BTreeMap<PlateId, Vec<RotationSequence>>,
}

impl RotationModel {
    /// Read PLATES4 `.rot` text; moving plate 999 and comment lines are disabled.
    pub fn from_rot(text: &str) -> Result<Self, PlateModelError> {
        Self::from_sequences(parser::parse(text)?)
    }

    /// Validate histories, ordering, and overlaps before building the lookup index.
    pub fn from_sequences(sequences: Vec<RotationSequence>) -> Result<Self, PlateModelError> {
        let sequences = sequence::index(sequences)?;
        Ok(Self { sequences })
    }

    /// Validated sequences in moving-plate and start-age order.
    pub fn sequences(&self) -> impl Iterator<Item = &RotationSequence> {
        self.sequences.values().flatten()
    }

    /// Rotate a present-day anchor without changing the stored observation.
    pub fn reconstruct(
        &self,
        point: LonLat,
        moving: PlateId,
        age_ma: f64,
        anchor: PlateId,
    ) -> Result<LonLat, PlateModelError> {
        self.rotation(moving, age_ma, anchor)?
            .rotation
            .apply(point)
            .map_err(PlateModelError::Rotation)
    }
}

impl PlateMotion for RotationModel {
    fn rotation(
        &self,
        moving: PlateId,
        age_ma: f64,
        anchor: PlateId,
    ) -> Result<PlateRotation, PlateModelError> {
        if !age_ma.is_finite() || age_ma < 0.0 {
            return Err(PlateModelError::Age(age_ma));
        }
        let (moving_rotation, circuit) = self.circuit(moving, age_ma)?;
        let (anchor_rotation, anchor_circuit) = self.circuit(anchor, age_ma)?;
        let rotation = moving_rotation.then(anchor_rotation.inverse()?)?;
        Ok(PlateRotation {
            rotation,
            circuit,
            anchor_circuit,
        })
    }
}

#[cfg(test)]
mod tests;
