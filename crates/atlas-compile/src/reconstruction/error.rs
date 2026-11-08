use atlas_geo::{plates::PlateModelError, rotation::RotationError};
use atlas_ledger::validation::ValidationError;
use thiserror::Error;

use crate::{export::ExportError, input::InputError, plate_models::PlateDocumentError};

/// Invalid reconstruction evidence, query, or artifact output.
#[derive(Debug, Error)]
pub enum ReconstructionError {
    /// Ledger or job file could not be read.
    #[error("reconstruction input: {0}")]
    Input(#[from] InputError),
    /// Ledger evidence failed validation.
    #[error("reconstruction ledger: {0}")]
    Validation(#[from] ValidationError),
    /// Model provenance or coverage failed validation.
    #[error("reconstruction model: {0}")]
    Model(#[from] PlateDocumentError),
    /// Unresolved observation or plate assignment.
    #[error("reconstruction evidence: {0}")]
    Evidence(String),
    /// Epoch difference overflowed the atlas numerical range.
    #[error("reconstruction epoch difference exceeds the millisecond range")]
    Epoch,
    /// Query falls outside the declared model excerpt.
    #[error("age {age_ma} Ma falls outside model coverage [{youngest_ma}, {oldest_ma}] Ma")]
    Age {
        /// Requested age in Ma.
        age_ma: f64,
        /// Youngest available age in Ma.
        youngest_ma: f64,
        /// Oldest available age in Ma.
        oldest_ma: f64,
    },
    /// Rotation circuit could not be resolved.
    #[error("reconstruction circuit: {0}")]
    Circuit(#[from] PlateModelError),
    /// Geographic transform failed.
    #[error("reconstructed coordinate: {0}")]
    Rotation(#[from] RotationError),
    /// Static export failed.
    #[error("reconstruction export: {0}")]
    Export(#[from] ExportError),
}
