use super::{BoundaryError, BoundaryRegistration, SheetTrace, TracedUnit};
use crate::{
    input::LedgerInput,
    registration::{RegisteredSheet, RegistrationJob},
};
use atlas_boundary::polygon::{densify, resolve};

pub(super) fn register(
    input: &LedgerInput,
    trace: &SheetTrace,
    controls: BoundaryRegistration,
) -> Result<(RegisteredSheet, Vec<TracedUnit>), BoundaryError> {
    let polygons: Vec<_> = trace
        .units
        .iter()
        .flat_map(|unit| unit.polygons.iter().cloned())
        .collect();
    let (vertices, expanded) =
        densify(&trace.vertices, &polygons, controls.maximum_segment_pixels)?;
    let sheet = crate::registration::register(
        input,
        RegistrationJob {
            source_id: trace.source_id.clone(),
            scan_page: trace.scan_page,
            source_frame: trace.source_frame,
            target_frame: controls.target_frame,
            method: controls.method,
            controls: controls.controls,
            check_points: controls.check_points,
            vertices,
        },
    )?;
    let mut remaining = expanded.into_iter();
    let mut units = Vec::new();
    for unit in &trace.units {
        let polygons: Vec<_> = remaining.by_ref().take(unit.polygons.len()).collect();
        resolve(&sheet.vertices, &polygons).map_err(|source| BoundaryError::Topology {
            record_id: unit.record_id,
            frame: "registered coordinates",
            source,
        })?;
        units.push(TracedUnit {
            record_id: unit.record_id,
            polygons,
        });
    }
    Ok((sheet, units))
}
