//! Publication integrity, time selection, and renderer errors.

use atlas_core::identity::EntityId;
use thiserror::Error;

/// Atlas input or time selection failure.
#[derive(Debug, Error)]
pub enum AppError {
    /// Terrain reconstruction geometry or source evidence is invalid.
    #[error(transparent)]
    Derivation(#[from] atlas_boundary::terrain::TerrainError),
    /// Registered boundary geometry or its evidence failed validation.
    #[error(transparent)]
    Boundary(#[from] atlas_boundary::geographic::GeographicError),
    /// Source-sheet geometry or citation validation failed.
    #[error(transparent)]
    Sheet(#[from] atlas_boundary::sheet::SheetError),
    /// Territorial relationship selection failed.
    #[error("territory: {0}")]
    Territory(#[from] atlas_boundary::nodal::NodalError),
    /// Map renderer failed.
    #[error(transparent)]
    Render(#[from] crate::scene::renderer::RenderError),
    /// A publication document could not be decoded.
    #[error("atlas JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The entry key differs from the entity it contains.
    #[error("entity shard key {key:?} contains {entity:?}")]
    EntryIdentity {
        /// Published map key.
        key: EntityId,
        /// Entity within the entry.
        entity: EntityId,
    },
    /// A record or estimate is attached to another entity's entry.
    #[error("entry {0:?} contains evidence for another entity")]
    EvidenceIdentity(EntityId),
    /// Published input includes a transcription without review.
    #[error("entry {0:?} contains an unchecked record")]
    Unchecked(EntityId),
    /// Evidence validation failed.
    #[error("atlas evidence: {0}")]
    Validation(#[from] atlas_ledger::validation::ValidationError),
    /// An exact year, age, or tick could not be parsed.
    #[error("invalid time: {0}")]
    Number(#[from] std::num::ParseIntError),
    /// Date input is missing a year, month, or day.
    #[error("invalid date {0:?}; use YYYY-MM-DD")]
    DateFormat(String),
    /// Calendar date is invalid.
    #[error("calendar date: {0}")]
    Calendar(#[from] atlas_core::calendar::CalendarError),
    /// Time falls outside the continuous axis.
    #[error("time selection: {0}")]
    Time(#[from] atlas_core::time::TimeError),
}
