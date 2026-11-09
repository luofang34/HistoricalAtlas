use super::{BoundaryError, SheetTrace};
use crate::input::LedgerInput;
use atlas_core::identity::FactId;
use atlas_ledger::model::Record;

pub(super) fn record(input: &LedgerInput, id: FactId) -> Result<&Record, BoundaryError> {
    input
        .data
        .records
        .iter()
        .find(|record| record.id == id && record.checked)
        .ok_or_else(|| BoundaryError::Evidence(format!("missing checked record {id:?}")))
}

pub(super) fn fingerprint<'a>(
    input: &'a LedgerInput,
    trace: &SheetTrace,
) -> Result<&'a str, BoundaryError> {
    let source = input
        .catalog
        .sources
        .iter()
        .find(|source| source.id == trace.source_id)
        .ok_or_else(|| BoundaryError::Evidence(format!("unknown source {}", trace.source_id)))?;
    source.raster_fingerprint(trace.scan_page).ok_or_else(|| {
        BoundaryError::Evidence(format!(
            "{} raster {:?} has no fingerprint",
            source.id, trace.scan_page
        ))
    })
}

/// Check source citations, raster bounds, construction evidence, and polygon topology.
pub fn validate_trace(input: &LedgerInput, trace: &SheetTrace) -> Result<(), BoundaryError> {
    atlas_boundary::sheet::validate(&input.catalog, &input.data, trace)?;
    Ok(())
}
