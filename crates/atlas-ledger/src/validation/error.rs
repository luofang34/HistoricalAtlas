use atlas_core::{identity::InvalidKey, time::TimeError};
use atlas_geo::coordinates::CoordinateError;
use thiserror::Error;

/// A ledger integrity violation, including the offending key or citation.
#[derive(Debug, Error)]
#[error("{context}: {problem}")]
pub struct ValidationError {
    /// Source, entity, or record key.
    pub context: String,
    /// Reason validation failed.
    pub problem: String,
    /// Underlying typed failure, when validation delegates to a domain model.
    #[source]
    pub source: Option<ValidationCause>,
}

/// Domain errors retained when validating ledger evidence.
#[derive(Debug, Error)]
pub enum ValidationCause {
    /// Semantic key failure.
    #[error(transparent)]
    Identity(#[from] InvalidKey),
    /// Spatial evidence failure.
    #[error(transparent)]
    Coordinate(#[from] CoordinateError),
    /// Temporal evidence failure.
    #[error(transparent)]
    Time(#[from] TimeError),
}

pub(crate) fn fail(context: impl Into<String>, problem: impl Into<String>) -> ValidationError {
    ValidationError {
        context: context.into(),
        problem: problem.into(),
        source: None,
    }
}

pub(super) fn caused(
    context: impl Into<String>,
    source: impl Into<ValidationCause>,
) -> ValidationError {
    let source = source.into();
    ValidationError {
        context: context.into(),
        problem: source.to_string(),
        source: Some(source),
    }
}
