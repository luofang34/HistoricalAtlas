//! Cited plate-rotation excerpts and the paleogeographic positions they imply at a time.

use std::collections::BTreeMap;

use atlas_core::{
    identity::{EntityId, FactId},
    time::{EpochTime, JULIAN_YEAR_MS, TimeError},
};
use atlas_geo::{
    coordinates::LonLat,
    plates::{PlateId, PlateModelError, RotationModel, RotationSequence},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    model::{Dataset, EstimateValue},
    timeline::TimeSelection,
};

/// A reproducible subset of a cited rotation file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlateModelDocument {
    /// Stable model name used by plate-assignment estimates and artifact paths.
    pub id: String,
    /// Catalogued rotation-file edition.
    pub source_id: String,
    /// SHA-256 fingerprint of the complete source rotation file.
    pub source_sha256: String,
    /// Data creator and associated research citation.
    pub attribution: String,
    /// License governing the source rotation data.
    pub license: String,
    /// Definition of the absolute frame, whose anchor is plate zero.
    pub reference_frame: String,
    /// Atlas epoch assigned to model age zero; ages use Julian million years.
    pub present_epoch: EpochTime,
    /// Inclusive youngest age covered by this excerpt, in Ma.
    pub youngest_ma: f64,
    /// Inclusive oldest age covered by this excerpt, in Ma.
    pub oldest_ma: f64,
    /// Extracted rotations, retaining line numbers in the complete source file.
    pub sequences: Vec<RotationSequence>,
}

impl PlateModelDocument {
    /// Model age of an epoch, when the excerpt covers it.
    pub fn age_at(&self, at: EpochTime) -> Option<f64> {
        let elapsed = self.present_epoch.ticks().checked_sub(at.ticks())?;
        let age_ma = elapsed as f64 / (JULIAN_YEAR_MS as f64 * 1_000_000.0);
        (self.youngest_ma..=self.oldest_ma)
            .contains(&age_ma)
            .then_some(age_ma)
    }
}

/// A source coordinate rotated with its entity's cited plate assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaleoPosition {
    /// Entity whose coordinate moved.
    pub entity_id: EntityId,
    /// Coordinate observation that was rotated, unchanged in the ledger.
    pub record_id: FactId,
    /// Plate-assignment estimate supplying the moving plate.
    pub plate_estimate: FactId,
    /// Rotation model.
    pub model_id: String,
    /// Moving plate.
    pub plate_id: PlateId,
    /// Model age of the query epoch, in Julian million years.
    pub age_ma: f64,
    /// Frame of the result: the model's absolute frame anchored on plate zero.
    pub reference_frame: String,
    /// Present-day coordinate from the observation.
    pub observed: LonLat,
    /// Reconstructed coordinate at the query epoch.
    pub reconstructed: LonLat,
}

/// Plate-assignment validity or rotation-circuit failure during paleogeographic reconstruction.
#[derive(Debug, Error)]
pub enum PaleoError {
    /// A cited rotation model could not be built or could not reconstruct a coordinate.
    #[error(transparent)]
    Plate(#[from] PlateModelError),
    /// A plate-assignment temporal validity span failed validation.
    #[error("plate assignment validity: {0}")]
    Time(#[from] TimeError),
}

/// Rotate every checked coordinate of plate-assigned entities whose model covers `at`.
///
/// Entities without an assignment, or whose model excerpt does not cover the
/// epoch, are left out rather than shown at present-day positions as if
/// they were reconstructions.
pub fn paleo_positions(
    models: &BTreeMap<String, PlateModelDocument>,
    data: &Dataset,
    at: EpochTime,
) -> Result<Vec<PaleoPosition>, PaleoError> {
    paleo_positions_with_models(models, &BTreeMap::new(), data, at, TimeSelection::Nominal)
}

/// Rotate checked coordinates using pre-built rotation models and a time-selection mode.
///
/// Optional `validity` spans on plate assignments restrict reconstructions to epochs
/// selected by `selection`, allowing crust formation, terrane transfer, and subduction.
pub fn paleo_positions_with_models(
    models: &BTreeMap<String, PlateModelDocument>,
    built: &BTreeMap<String, RotationModel>,
    data: &Dataset,
    at: EpochTime,
    selection: TimeSelection,
) -> Result<Vec<PaleoPosition>, PaleoError> {
    let mut cache: BTreeMap<&str, RotationModel> = BTreeMap::new();
    let mut positions = Vec::new();
    for estimate in &data.estimates {
        let EstimateValue::Plate {
            model_id,
            plate_id,
            validity,
        } = &estimate.value
        else {
            continue;
        };
        if let Some(span) = validity
            && selection.select(Some(span), at)?.is_none()
        {
            continue;
        }
        let Some(document) = models.get(model_id) else {
            continue;
        };
        let Some(age_ma) = document.age_at(at) else {
            continue;
        };
        let model = if let Some(existing) = built.get(model_id) {
            existing
        } else {
            if !cache.contains_key(model_id.as_str()) {
                let compiled = RotationModel::from_sequences(document.sequences.clone())?;
                cache.insert(model_id.as_str(), compiled);
            }
            let Some(compiled) = cache.get(model_id.as_str()) else {
                continue;
            };
            compiled
        };
        for record in data
            .records
            .iter()
            .filter(|record| record.checked && record.entity_id == estimate.entity_id)
        {
            let Some(coordinate) = &record.observation.coordinate else {
                continue;
            };
            let reconstructed =
                model.reconstruct(coordinate.point, *plate_id, age_ma, PlateId(0))?;
            positions.push(PaleoPosition {
                entity_id: estimate.entity_id,
                record_id: record.id,
                plate_estimate: estimate.id,
                model_id: model_id.clone(),
                plate_id: *plate_id,
                age_ma,
                reference_frame: document.reference_frame.clone(),
                observed: coordinate.point,
                reconstructed,
            });
        }
    }
    Ok(positions)
}

#[cfg(test)]
mod tests;
