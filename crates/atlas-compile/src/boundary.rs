//! Cited map traces and registered boundaries with separate map and fit provenance.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{
    export::{Artifacts, compile, write_blocking},
    input::{LedgerInput, load_blocking, read_json_blocking},
    registration::{ReferencedControl, RegistrationMethod, TargetFrame},
};

mod error;
mod evidence;
mod publication;
mod transform;
pub use error::BoundaryError;
pub use evidence::validate_trace;

pub use atlas_boundary::sheet::{BoundaryMethod, SheetTrace, TracedPlace, TracedUnit};

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
    validate_trace(input, &trace)?;
    let spacing = controls.maximum_segment_pixels;
    let transform::RegisteredTrace {
        sheet,
        units,
        lines,
    } = transform::register(input, &trace, controls)?;
    let boundaries = publication::geographic_boundaries(&trace, &sheet, &units)?;
    let linework = publication::geographic_lines(&sheet, &lines)?;
    let mut artifacts = crate::export::compile_registered(input, &boundaries, &linework)?;
    publication::write_trace(&mut artifacts, input, &trace)?;
    publication::write_registered(
        &mut artifacts,
        input,
        &trace,
        &sheet,
        &units,
        &lines,
        spacing,
    )?;
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
