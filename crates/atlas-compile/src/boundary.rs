//! Cited map traces and registered boundaries with separate map and fit provenance.

use atlas_boundary::polygon::IndexedPolygon;
use atlas_core::identity::FactId;
use atlas_geo::registration::PlanarPoint;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{
    export::{Artifacts, compile, write_blocking},
    input::{LedgerInput, load_blocking, read_json_blocking},
    registration::{RasterFrame, ReferencedControl, RegistrationMethod, TargetFrame},
};

mod error;
mod evidence;
mod publication;
mod transform;
pub use error::BoundaryError;
pub use evidence::validate_trace;

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
    /// Record identifying the subject, edition, sheet, and depicted boundary.
    pub record_id: FactId,
    /// Components sharing the sheet's pixel vertex array.
    pub polygons: Vec<IndexedPolygon>,
}

/// Reviewed linework in a fingerprinted scan's full-page pixel frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetTrace {
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
    /// Entity boundaries referencing the shared vertices.
    pub units: Vec<TracedUnit>,
}

/// Controls and sampling parameters for georeferencing a reviewed trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryRegistration {
    /// CRS and units shared by every target control.
    pub target_frame: TargetFrame,
    /// Affine or thin-plate registration.
    pub method: RegistrationMethod,
    /// Referenced fitting landmarks.
    pub controls: Vec<ReferencedControl>,
    /// Referenced holdouts excluded from fitting.
    pub check_points: Vec<ReferencedControl>,
    /// Maximum source-pixel segment length before transforming linework.
    pub maximum_segment_pixels: f64,
}

/// Compile a source-pixel trace without assigning geographical coordinates.
pub fn compile_trace(input: &LedgerInput, trace: SheetTrace) -> Result<Artifacts, BoundaryError> {
    let mut artifacts = compile(input)?;
    validate_trace(input, &trace)?;
    publication::write_trace(&mut artifacts, input, &trace)?;
    Ok(artifacts)
}

/// Register a source trace, validate transformed topology, and compile its evidence.
pub fn compile_boundary(
    input: &LedgerInput,
    trace: SheetTrace,
    controls: BoundaryRegistration,
) -> Result<Artifacts, BoundaryError> {
    let mut artifacts = compile_trace(input, trace.clone())?;
    let spacing = controls.maximum_segment_pixels;
    let (sheet, units) = transform::register(input, &trace, controls)?;
    publication::write_registered(&mut artifacts, input, &trace, &sheet, &units, spacing)?;
    Ok(artifacts)
}

/// Read a trace and optional controls, then publish into a new output directory.
pub fn boundary_blocking(
    root: &Path,
    trace: &Path,
    controls: Option<&Path>,
    output: &Path,
) -> Result<usize, BoundaryError> {
    let input = load_blocking(root)?;
    let trace = read_json_blocking(trace)?;
    let artifacts = match controls {
        Some(path) => compile_boundary(&input, trace, read_json_blocking(path)?)?,
        None => compile_trace(&input, trace)?,
    };
    write_blocking(output, &artifacts)?;
    Ok(artifacts.len())
}

#[cfg(test)]
mod tests;
