//! Reviewed source-sheet geometry with pixel coordinates and evidence links.

use crate::polygon::IndexedPolygon;
use atlas_core::identity::FactId;
use atlas_geo::registration::PlanarPoint;
use serde::{Deserialize, Serialize};

mod validation;
pub use validation::{SheetError, validate};

/// A named symbol measured on a source sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TracedPlace {
    /// Checked map observation identifying the depicted entity.
    pub record_id: FactId,
    /// Symbol center in the full-page pixel frame.
    pub point: PlanarPoint,
}

/// Dimensions of the full scan-page raster used to measure source points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterFrame {
    /// Image width in pixels, with x increasing to the right from the left edge.
    pub width: u32,
    /// Image height in pixels, with y increasing downwards from the top edge.
    pub height: u32,
}

impl RasterFrame {
    /// Whether a finite point lies inside the full raster.
    pub fn contains(self, point: PlanarPoint) -> bool {
        self.width > 0
            && self.height > 0
            && (0.0..=f64::from(self.width)).contains(&point.x)
            && (0.0..=f64::from(self.height)).contains(&point.y)
    }
}

/// How the source established its depicted administrative boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryMethod {
    /// The source explicitly identifies a surveyed boundary.
    Surveyed,
    /// The source compiled its line from the locations of subordinate places.
    CompiledFromPlaces,
    /// The source's boundary-construction method is unknown.
    Unstated,
}

/// Boundary polygons of one entity, linked to a checked sheet observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TracedUnit {
    /// Interval supported by the boundary observation; publication date alone does not date geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validity: Option<atlas_core::time::TemporalSpan>,
    /// Record identifying the subject, edition, sheet, and depicted boundary.
    pub record_id: FactId,
    /// Components sharing the sheet's pixel vertex array.
    pub polygons: Vec<IndexedPolygon>,
}

/// Reviewed linework in a fingerprinted scan's full-page pixel frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetTrace {
    /// Explicit registration identifier when one page contains several maps or solutions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration_id: Option<String>,
    /// Bibliographic source edition.
    pub source_id: String,
    /// One-based scan page containing the map.
    pub scan_page: u32,
    /// Dimensions of the full-page raster used for tracing.
    pub source_frame: RasterFrame,
    /// Source boundary construction, independent of georeferencing accuracy.
    pub boundary_method: BoundaryMethod,
    /// Checked same-edition records establishing the construction method.
    pub method_basis: Vec<FactId>,
    /// Measured image vertices, with x rightward and y downward.
    pub vertices: Vec<PlanarPoint>,
    /// Reviewed places in the same full-page pixel frame.
    #[serde(default)]
    pub places: Vec<TracedPlace>,
    /// Entity boundaries referencing the shared vertices.
    pub units: Vec<TracedUnit>,
}
