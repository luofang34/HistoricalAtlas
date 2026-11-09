//! Evidence requirements for intervals asserted on geometry.

use atlas_core::{identity::FactId, time::TemporalSpan};
use atlas_ledger::model::Dataset;
use thiserror::Error;

/// Check that a geometry's interval is well ordered and cited by its subject's own evidence.
///
/// A sheet's publication date does not date the boundary it draws, so an
/// interval must cite checked records or estimates about the same entity as
/// the geometry's subject record; a basis without an interval is rejected too.
pub fn validate(
    data: &Dataset,
    subject: FactId,
    validity: Option<&TemporalSpan>,
    basis: &[FactId],
) -> Result<(), ValidityError> {
    let Some(span) = validity else {
        return if basis.is_empty() {
            Ok(())
        } else {
            Err(ValidityError::BasisWithoutInterval(subject))
        };
    };
    span.validate()
        .map_err(|error| ValidityError::Interval(error.to_string()))?;
    if basis.is_empty() {
        return Err(ValidityError::Uncited(subject));
    }
    let entity = data
        .records
        .iter()
        .find(|record| record.id == subject)
        .map(|record| record.entity_id)
        .ok_or(ValidityError::Subject(subject))?;
    for id in basis {
        let cited = data
            .records
            .iter()
            .find(|record| record.id == *id && record.checked)
            .map(|record| record.entity_id)
            .or_else(|| {
                data.estimates
                    .iter()
                    .find(|estimate| estimate.id == *id)
                    .map(|estimate| estimate.entity_id)
            });
        if cited != Some(entity) {
            return Err(ValidityError::Basis {
                subject,
                basis: *id,
            });
        }
    }
    Ok(())
}

/// Unsupported or malformed geometry interval.
#[derive(Debug, Error)]
pub enum ValidityError {
    /// The interval itself is malformed.
    #[error("geometry interval: {0}")]
    Interval(String),
    /// An interval was asserted without evidence.
    #[error("interval on {0:?} cites no evidence")]
    Uncited(FactId),
    /// Evidence was listed for a geometry without an interval.
    #[error("validity basis on {0:?} has no interval to support")]
    BasisWithoutInterval(FactId),
    /// The geometry's subject record is missing.
    #[error("geometry subject {0:?} is missing")]
    Subject(FactId),
    /// A cited fact is missing, unchecked, or about another entity.
    #[error(
        "validity basis {basis:?} for {subject:?} is missing, unchecked, or about another entity"
    )]
    Basis {
        /// Geometry subject record.
        subject: FactId,
        /// Offending cited fact.
        basis: FactId,
    },
}

#[cfg(test)]
mod tests;
