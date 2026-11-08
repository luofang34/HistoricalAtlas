use super::{BoundaryError, BoundaryMethod, SheetTrace};
use crate::input::LedgerInput;
use atlas_boundary::polygon::resolve;
use atlas_core::identity::FactId;
use atlas_ledger::model::Record;
use std::collections::BTreeSet;

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
    fingerprint(input, trace)?;
    if trace.units.is_empty()
        || trace.vertices.is_empty()
        || trace
            .vertices
            .iter()
            .any(|point| !trace.source_frame.contains(*point))
    {
        return Err(BoundaryError::Evidence(
            "trace requires units and vertices inside its full-page raster".into(),
        ));
    }
    if trace.boundary_method != BoundaryMethod::Unstated && trace.method_basis.is_empty() {
        return Err(BoundaryError::Evidence(
            "stated boundary method requires cited records".into(),
        ));
    }
    for id in &trace.method_basis {
        if record(input, *id)?.source_id != trace.source_id {
            return Err(BoundaryError::Evidence(format!(
                "method record {id:?} belongs to a different edition"
            )));
        }
    }
    let mut subjects = BTreeSet::new();
    let mut body = None;
    for unit in &trace.units {
        let record = record(input, unit.record_id)?;
        if record.source_id != trace.source_id || record.scan_page != Some(trace.scan_page) {
            return Err(BoundaryError::Evidence(format!(
                "{} does not cite the traced sheet",
                record.key
            )));
        }
        if !subjects.insert(record.entity_id) {
            return Err(BoundaryError::Evidence(format!(
                "duplicate boundary subject {:?}",
                record.entity_id
            )));
        }
        let entity = input
            .data
            .entities
            .iter()
            .find(|entity| entity.id == record.entity_id)
            .ok_or_else(|| {
                BoundaryError::Evidence(format!("missing entity {:?}", record.entity_id))
            })?;
        if body.is_some_and(|previous| previous != entity.body) {
            return Err(BoundaryError::Evidence(
                "one sheet cannot mix celestial bodies".into(),
            ));
        }
        body = Some(entity.body);
        resolve(&trace.vertices, &unit.polygons).map_err(|source| BoundaryError::Topology {
            record_id: unit.record_id,
            frame: "source pixels",
            source,
        })?;
    }
    Ok(())
}
