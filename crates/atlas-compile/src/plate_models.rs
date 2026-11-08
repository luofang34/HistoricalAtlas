//! Cited rotation excerpts and validation of ledger plate assignments.

use std::collections::BTreeMap;

use atlas_core::time::EpochTime;
use atlas_geo::plates::{PlateId, PlateModelError, PlateMotion, RotationModel, RotationSequence};
use atlas_ledger::model::{Dataset, EstimateValue, SourceCatalog};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    /// Check provenance, coverage limits, and finite-rotation structure.
    pub fn model(&self, catalog: &SourceCatalog) -> Result<RotationModel, PlateDocumentError> {
        let invalid = |reason: &str| PlateDocumentError::Invalid {
            model: self.id.clone(),
            reason: reason.into(),
        };
        if self.id.is_empty()
            || !self
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(invalid(
                "model ID must use lowercase ASCII letters, digits, and hyphens",
            ));
        }
        if !catalog
            .sources
            .iter()
            .any(|source| source.id == self.source_id)
            || self.source_sha256.len() != 64
            || !self
                .source_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || [&self.attribution, &self.license, &self.reference_frame]
                .iter()
                .any(|value| value.trim().is_empty())
        {
            return Err(invalid(
                "model needs a catalogued source, SHA-256, attribution, license, and reference frame",
            ));
        }
        if !self.youngest_ma.is_finite()
            || !self.oldest_ma.is_finite()
            || self.youngest_ma < 0.0
            || self.youngest_ma > self.oldest_ma
            || self.sequences.is_empty()
        {
            return Err(invalid(
                "model needs nonempty rotations and a finite nonnegative age interval",
            ));
        }
        RotationModel::from_sequences(self.sequences.clone()).map_err(|source| {
            PlateDocumentError::Rotation {
                model: self.id.clone(),
                source,
            }
        })
    }

    /// Include samples, crossovers, interval interiors, and excerpt endpoints.
    fn check_ages(&self) -> Vec<f64> {
        let mut boundaries = vec![self.youngest_ma, self.oldest_ma];
        boundaries.extend(
            self.sequences
                .iter()
                .flat_map(|sequence| &sequence.samples)
                .map(|sample| sample.age_ma)
                .filter(|age| self.youngest_ma <= *age && *age <= self.oldest_ma),
        );
        boundaries.sort_by(f64::total_cmp);
        boundaries.dedup();
        let mut ages = boundaries.clone();
        ages.extend(boundaries.windows(2).filter_map(|pair| {
            if let [young, old] = pair {
                Some(young + (old - young) / 2.0)
            } else {
                None
            }
        }));
        ages
    }
}

/// Validate model identity, attribution, and complete circuits for every assignment.
pub fn validate_models(
    models: &BTreeMap<String, PlateModelDocument>,
    catalog: &SourceCatalog,
    data: &Dataset,
) -> Result<(), PlateDocumentError> {
    for (id, document) in models {
        if id != &document.id {
            return Err(PlateDocumentError::Invalid {
                model: id.clone(),
                reason: "catalog key differs from model ID".into(),
            });
        }
        document.model(catalog)?;
    }
    for estimate in &data.estimates {
        let EstimateValue::Plate { model_id, plate_id } = &estimate.value else {
            continue;
        };
        let invalid = |reason: &str| PlateDocumentError::Invalid {
            model: model_id.clone(),
            reason: format!("{}: {reason}", estimate.key),
        };
        let document = models
            .get(model_id)
            .ok_or_else(|| invalid("unknown model"))?;
        if !data.records.iter().any(|record| {
            record.checked
                && record.source_id == document.source_id
                && estimate.basis.contains(&record.id)
        }) {
            return Err(invalid(
                "assignment basis must cite the rotation-file edition",
            ));
        }
        let model = document.model(catalog)?;
        for age in document.check_ages() {
            model
                .rotation(*plate_id, age, PlateId(0))
                .map_err(|source| PlateDocumentError::Rotation {
                    model: model_id.clone(),
                    source,
                })?;
        }
    }
    Ok(())
}

/// Missing model evidence or invalid rotation coverage.
#[derive(Debug, Error)]
pub enum PlateDocumentError {
    /// Model metadata or assignment evidence is invalid.
    #[error("plate model {model}: {reason}")]
    Invalid {
        /// Model identity.
        model: String,
        /// Broken provenance or identity requirement.
        reason: String,
    },
    /// A finite rotation or circuit is invalid.
    #[error("plate model {model}: {source}")]
    Rotation {
        /// Model identity.
        model: String,
        /// Numerical or sequence error.
        #[source]
        source: PlateModelError,
    },
}
