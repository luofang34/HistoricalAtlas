//! Plate reconstructions of cited coordinates at an atlas epoch.

use std::path::Path;

use atlas_core::{
    identity::{EntityId, FactId},
    time::{EpochTime, JULIAN_YEAR_MS},
};
use atlas_geo::{
    coordinates::{CoordinateObservation, LonLat},
    plates::{PlateId, PlateMotion, PlateRotation},
};
use atlas_ledger::model::{EstimateValue, Record};
use serde::{Deserialize, Serialize};

use crate::{
    export::{Artifacts, compile, insert, write_blocking},
    input::{LedgerInput, load_blocking, read_json_blocking},
};

mod error;
pub use error::ReconstructionError;

/// One coordinate observation paired with a model-specific plate assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconstructionAnchor {
    /// Checked record containing the present-day body-fixed coordinate.
    pub coordinate_record: FactId,
    /// Plate-assignment estimate for that same entity.
    pub plate_estimate: FactId,
}

/// Request to reconstruct source coordinates without altering their stored values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconstructionJob {
    /// Epoch on the atlas TT millisecond axis.
    pub at: EpochTime,
    /// Model to use for every requested anchor.
    pub model_id: String,
    /// Plate fixed in the output reference frame; zero uses the model's absolute frame.
    pub anchor_plate: PlateId,
    /// Coordinate and plate-assignment evidence pairs.
    pub anchors: Vec<ReconstructionAnchor>,
}

/// A derived position with its original observation and resolved rotation circuit.
#[derive(Debug, Serialize)]
pub struct ReconstructedAnchor {
    /// Enduring subject identity.
    pub entity_id: EntityId,
    /// Original coordinate record.
    pub coordinate_record: FactId,
    /// Chosen plate-assignment estimate.
    pub plate_estimate: FactId,
    /// Unmodified notation, precision, and datum from the cited observation.
    pub observation: CoordinateObservation,
    /// Longitude and latitude in the selected plate-model frame, in degrees.
    pub reconstructed: LonLat,
    /// Rotation and its moving/anchor source-line circuits.
    pub motion: PlateRotation,
}

/// Reconstruction result linked to the published model and evidence index.
#[derive(Debug, Serialize)]
pub struct ReconstructionResult {
    /// Original request, including epoch and evidence selections.
    pub job: ReconstructionJob,
    /// Age used to query the model, in Julian million years before its present epoch.
    pub age_ma: f64,
    /// Fingerprint of the cited complete rotation source.
    pub model_source_sha256: String,
    /// Derived positions in input order.
    pub anchors: Vec<ReconstructedAnchor>,
}

/// Resolve a cited set of anchors against one validated finite-rotation model.
pub fn reconstruct(
    input: &LedgerInput,
    job: ReconstructionJob,
) -> Result<ReconstructionResult, ReconstructionError> {
    atlas_ledger::validation::validate(&input.catalog, &input.data)?;
    crate::plate_models::validate_models(&input.plate_models, &input.catalog, &input.data)?;
    let document = input
        .plate_models
        .get(&job.model_id)
        .ok_or_else(|| ReconstructionError::Evidence(format!("unknown model {}", job.model_id)))?;
    let elapsed = document
        .present_epoch
        .ticks()
        .checked_sub(job.at.ticks())
        .ok_or(ReconstructionError::Epoch)?;
    let age_ma = elapsed as f64 / (JULIAN_YEAR_MS as f64 * 1_000_000.0);
    if !(document.youngest_ma..=document.oldest_ma).contains(&age_ma) {
        return Err(ReconstructionError::Age {
            age_ma,
            youngest_ma: document.youngest_ma,
            oldest_ma: document.oldest_ma,
        });
    }
    if job.anchors.is_empty() {
        return Err(ReconstructionError::Evidence(
            "reconstruction needs at least one coordinate anchor".into(),
        ));
    }
    let model = crate::plate_models::validated_model(document, &input.catalog)?;
    let mut anchors = Vec::new();
    for anchor in &job.anchors {
        let (record, plate) = resolve_anchor(input, anchor, &job.model_id)?;
        let observation = record.observation.coordinate.clone().ok_or_else(|| {
            ReconstructionError::Evidence(format!("{} has no coordinate", record.key))
        })?;
        let motion = model.rotation(plate, age_ma, job.anchor_plate)?;
        let reconstructed = motion.rotation.apply(observation.point)?;
        anchors.push(ReconstructedAnchor {
            entity_id: record.entity_id,
            coordinate_record: anchor.coordinate_record,
            plate_estimate: anchor.plate_estimate,
            observation,
            reconstructed,
            motion,
        });
    }
    Ok(ReconstructionResult {
        job,
        age_ma,
        model_source_sha256: document.source_sha256.clone(),
        anchors,
    })
}

fn resolve_anchor<'a>(
    input: &'a LedgerInput,
    anchor: &ReconstructionAnchor,
    model: &str,
) -> Result<(&'a Record, PlateId), ReconstructionError> {
    let record = input
        .data
        .records
        .iter()
        .find(|record| record.id == anchor.coordinate_record && record.checked)
        .ok_or_else(|| {
            ReconstructionError::Evidence(format!(
                "coordinate record {:?} is missing or unchecked",
                anchor.coordinate_record
            ))
        })?;
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|estimate| {
            estimate.id == anchor.plate_estimate && estimate.entity_id == record.entity_id
        })
        .ok_or_else(|| {
            ReconstructionError::Evidence(format!(
                "plate estimate {:?} does not identify the coordinate subject",
                anchor.plate_estimate
            ))
        })?;
    match &estimate.value {
        EstimateValue::Plate { model_id, plate_id } if model_id == model => Ok((record, *plate_id)),
        _ => Err(ReconstructionError::Evidence(format!(
            "{} is not a plate assignment in {model}",
            estimate.key
        ))),
    }
}

/// Build a complete reproducible artifact set, including all model and citation files.
pub fn compile_reconstruction(
    input: &LedgerInput,
    job: ReconstructionJob,
) -> Result<Artifacts, ReconstructionError> {
    let result = reconstruct(input, job)?;
    let mut output = compile(input)?;
    insert(&mut output, "reconstruction.json", &result)?;
    Ok(output)
}

/// Read a request and write a new static reconstruction directory.
pub fn reconstruct_blocking(
    root: &Path,
    request: &Path,
    output: &Path,
) -> Result<usize, ReconstructionError> {
    let input = load_blocking(root)?;
    let job = read_json_blocking(request)?;
    let artifacts = compile_reconstruction(&input, job)?;
    write_blocking(output, &artifacts)?;
    Ok(artifacts.len())
}

#[cfg(test)]
mod tests;
