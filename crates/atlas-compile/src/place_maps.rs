//! Register depicted places against cited coordinate observations.

use atlas_core::identity::FactId;
use atlas_geo::{coordinates::Datum, registration::PlanarPoint};
use atlas_ledger::model::Estimate;
use serde::{Deserialize, Serialize};

use crate::{input::LedgerInput, registration::RegisteredSheet};

mod evidence;
mod transform;
pub use evidence::PlaceMapError;

/// A measured symbol linked to its checked map observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappedPlace {
    /// Record citing the symbol on this sheet.
    pub record_id: FactId,
    /// Full-page raster pixels, with x rightward and y downward.
    pub point: PlanarPoint,
}

/// A depicted place and the independent table coordinate used to locate it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceControl {
    /// Map observation whose measured symbol supplies source pixels.
    pub map_record_id: FactId,
    /// Checked coordinate observation of the same entity.
    pub coordinate_record_id: FactId,
}

/// Reproducible affine registration of one map's named places.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceMap {
    /// Lowercase ASCII publication name.
    pub id: String,
    /// Bibliographic edition containing the sheet.
    pub source_id: String,
    /// One-based scan page.
    pub scan_page: u32,
    /// Dimensions of the full unrotated scan raster.
    pub source_frame: crate::registration::RasterFrame,
    /// Datum shared by every fitting and checking coordinate.
    pub datum: Datum,
    /// Reviewed symbols, each citing its own sheet record.
    pub places: Vec<MappedPlace>,
    /// Correspondences included in the affine fit.
    pub controls: Vec<PlaceControl>,
    /// Correspondences excluded from the fit.
    pub check_points: Vec<PlaceControl>,
    /// Working positional allowance in metres.
    pub uncertainty_m: f64,
    /// Evidence and calculation supporting the positional allowance.
    pub uncertainty_basis: String,
}

/// Fit the map and derive locations without replacing coordinate transcriptions.
pub fn register_places(
    input: &LedgerInput,
    map: &PlaceMap,
) -> Result<(RegisteredSheet, Vec<Estimate>), PlaceMapError> {
    evidence::validate(input, map)?;
    transform::register(input, map)
}

#[cfg(test)]
mod tests;
