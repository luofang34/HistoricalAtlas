use atlas_core::identity::FactId;
use thiserror::Error;

/// Invalid tile publication parameters, coordinate, ledger, or output.
#[derive(Debug, Error)]
pub enum TileError {
    /// A feature exceeds the bounded tile generation budget.
    #[error(
        "geometry covers {count} candidate tiles at zoom {zoom}; reduce maximum zoom or partition the feature"
    )]
    Coverage {
        /// Requested zoom.
        zoom: u8,
        /// Candidate tile count.
        count: i64,
    },
    /// Unsupported or inconsistent feature geometry.
    #[error("tile geometry: {0}")]
    Geometry(&'static str),
    /// Publication address could not be parsed.
    #[error("invalid publication URL: {0}")]
    Url(#[from] url::ParseError),
    /// Publication URL has an unsuitable scheme, credentials, query, or fragment.
    #[error(
        "publication base must be an absolute HTTP(S) URL without credentials, query, or fragment"
    )]
    PublicationBase,
    /// Tile zoom exceeds the XYZ addressing domain.
    #[error("maximum tile zoom {0} exceeds 30")]
    Zoom(u8),
    /// A source coordinate lies outside the spherical Mercator display domain.
    #[error("record {record:?} latitude {latitude} exceeds the Mercator limit ±85.0511287798066°")]
    PolarCoordinate {
        /// Coordinate observation requiring a polar projection.
        record: FactId,
        /// Original source latitude.
        latitude: f64,
    },
    /// Ledger document loading failed.
    #[error("tile input: {0}")]
    Input(#[from] crate::input::InputError),
    /// Evidence validation or output writing failed.
    #[error("tile publication: {0}")]
    Export(#[from] crate::export::ExportError),
    /// An evidence field could not be represented in JSON.
    #[error("tile property serialization: {0}")]
    Json(#[from] serde_json::Error),
}
