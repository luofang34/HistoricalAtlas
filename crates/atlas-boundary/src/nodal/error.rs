use atlas_core::{identity::EntityId, time::TimeError};
use atlas_ledger::validation::ValidationError;
use thiserror::Error;

/// Missing query evidence or invalid territorial inputs.
#[derive(Debug, Error)]
pub enum NodalError {
    /// The input ledger failed validation.
    #[error("territory evidence: {0}")]
    Validation(#[from] ValidationError),
    /// The requested root has no identity in this ledger.
    #[error("unknown territory entity {0:?}")]
    UnknownEntity(EntityId),
    /// The requested source edition is absent.
    #[error("unknown territory source {0}")]
    UnknownSource(String),
    /// A selected interval cannot be evaluated.
    #[error("territory time selection: {0}")]
    Time(#[from] TimeError),
}
