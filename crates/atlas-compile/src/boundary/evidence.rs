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
    if trace.scan_page == 0
        || source
            .scan_pages
            .is_none_or(|total| trace.scan_page > total)
    {
        return Err(BoundaryError::Evidence(format!(
            "{} scan page {} out of range",
            source.id, trace.scan_page
        )));
    }
    source
        .scan_sha256
        .as_deref()
        .ok_or_else(|| BoundaryError::Evidence(format!("{} has no scan fingerprint", source.id)))
}

/// Check source citations, raster bounds, construction evidence, and polygon topology.
pub fn validate_trace(input: &LedgerInput, trace: &SheetTrace) -> Result<(), BoundaryError> {
    atlas_boundary::sheet::validate(&input.catalog, &input.data, trace)?;
    Ok(())
}
