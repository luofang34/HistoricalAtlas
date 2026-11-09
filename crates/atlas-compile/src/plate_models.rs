//! Cited rotation excerpts and validation of ledger plate assignments.

use std::collections::BTreeMap;

use atlas_geo::plates::{PlateId, PlateModelError, PlateMotion, RotationModel};
use atlas_ledger::model::{Dataset, EstimateValue, SourceCatalog};
use thiserror::Error;

pub use atlas_ledger::plates::PlateModelDocument;

/// Check provenance, coverage limits, and finite-rotation structure.
pub fn validated_model(
    document: &PlateModelDocument,
    catalog: &SourceCatalog,
) -> Result<RotationModel, PlateDocumentError> {
    let invalid = |reason: &str| PlateDocumentError::Invalid {
        model: document.id.clone(),
        reason: reason.into(),
    };
    if document.id.is_empty()
        || !document
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
        .any(|source| source.id == document.source_id)
        || document.source_sha256.len() != 64
        || !document
            .source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || [
            &document.attribution,
            &document.license,
            &document.reference_frame,
        ]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(invalid(
            "model needs a catalogued source, SHA-256, attribution, license, and reference frame",
        ));
    }
    if !document.youngest_ma.is_finite()
        || !document.oldest_ma.is_finite()
        || document.youngest_ma < 0.0
        || document.youngest_ma > document.oldest_ma
        || document.sequences.is_empty()
    {
        return Err(invalid(
            "model needs nonempty rotations and a finite nonnegative age interval",
        ));
    }
    RotationModel::from_sequences(document.sequences.clone()).map_err(|source| {
        PlateDocumentError::Rotation {
            model: document.id.clone(),
            source,
        }
    })
}

/// Include samples, crossovers, interval interiors, and excerpt endpoints.
fn check_ages(document: &PlateModelDocument) -> Vec<f64> {
    let mut boundaries = vec![document.youngest_ma, document.oldest_ma];
    boundaries.extend(
        document
            .sequences
            .iter()
            .flat_map(|sequence| &sequence.samples)
            .map(|sample| sample.age_ma)
            .filter(|age| document.youngest_ma <= *age && *age <= document.oldest_ma),
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
        validated_model(document, catalog)?;
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
        let model = validated_model(document, catalog)?;
        for age in check_ages(document) {
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
