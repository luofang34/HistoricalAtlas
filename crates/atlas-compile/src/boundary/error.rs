use crate::{export::ExportError, input::InputError, registration::RegistrationJobError};
use atlas_boundary::polygon::TopologyError;
use atlas_core::identity::FactId;
use thiserror::Error;

/// Evidence, geometry, registration, or publication failure for a boundary.
#[derive(Debug, Error)]
pub enum BoundaryError {
    /// Registered linework or its source evidence is invalid.
    #[error(transparent)]
    Line(#[from] atlas_boundary::linear::LineError),
    /// Registered geographic boundary or its evidence is invalid.
    #[error(transparent)]
    Geographic(#[from] atlas_boundary::geographic::GeographicError),
    /// Reviewed source-sheet geometry or evidence is invalid.
    #[error(transparent)]
    Sheet(#[from] atlas_boundary::sheet::SheetError),
    /// Request or ledger failed to load.
    #[error("boundary input: {0}")]
    Input(#[from] InputError),
    /// Source or record linkage is incomplete or inconsistent.
    #[error("boundary evidence: {0}")]
    Evidence(String),
    /// The boundary's source or transformed polygons are invalid.
    #[error("boundary record {record_id:?}, {frame}: {source}")]
    Topology {
        /// Source assertion identifying the affected entity.
        record_id: FactId,
        /// Coordinate frame being checked.
        frame: &'static str,
        /// Underlying topology error.
        #[source]
        source: TopologyError,
    },
    /// Edge subdivision failed.
    #[error("boundary sampling: {0}")]
    Sampling(#[from] TopologyError),
    /// Referenced registration failed.
    #[error("boundary registration: {0}")]
    Registration(#[from] RegistrationJobError),
    /// A static artifact failed validation or writing.
    #[error("boundary export: {0}")]
    Export(#[from] ExportError),
}
