use super::{BoundaryMethod, SheetTrace};
use crate::polygon::{TopologyError, resolve};
use atlas_core::identity::FactId;
use atlas_ledger::model::{Dataset, Record, SourceCatalog};
use std::collections::BTreeSet;
use thiserror::Error;

/// Invalid source-sheet geometry or citation.
#[derive(Debug, Error)]
pub enum SheetError {
    /// Open line geometry is invalid.
    #[error(transparent)]
    Line(#[from] crate::linear::LineError),
    /// Source evidence or pixel frame is inconsistent.
    #[error("source sheet: {0}")]
    Evidence(String),
    /// A boundary has invalid polygon topology.
    #[error("source-sheet boundary {record_id:?}: {source}")]
    Topology {
        /// Checked boundary observation.
        record_id: FactId,
        /// Underlying geometry failure.
        #[source]
        source: TopologyError,
    },
}

fn record(data: &Dataset, id: FactId) -> Result<&Record, SheetError> {
    data.records
        .iter()
        .find(|r| r.id == id && r.checked)
        .ok_or_else(|| SheetError::Evidence(format!("missing checked record {id:?}")))
}

/// Validate geometry and its checked sheet and method evidence.
pub fn validate(
    catalog: &SourceCatalog,
    data: &Dataset,
    trace: &SheetTrace,
) -> Result<(), SheetError> {
    let source = catalog
        .sources
        .iter()
        .find(|s| s.id == trace.source_id)
        .ok_or_else(|| SheetError::Evidence(format!("unknown source {}", trace.source_id)))?;
    if source.raster_fingerprint(trace.scan_page).is_none() {
        return Err(SheetError::Evidence(
            "sheet requires an identified source raster".into(),
        ));
    }
    if (trace.units.is_empty() && trace.lines.is_empty())
        || trace.vertices.is_empty()
        || trace
            .vertices
            .iter()
            .chain(trace.places.iter().map(|p| &p.point))
            .any(|point| !trace.source_frame.contains(*point))
    {
        return Err(SheetError::Evidence(
            "sheet requires geometry inside its raster".into(),
        ));
    }
    if trace.boundary_method != BoundaryMethod::Unstated && trace.method_basis.is_empty() {
        return Err(SheetError::Evidence(
            "stated boundary method requires cited records".into(),
        ));
    }
    for id in &trace.method_basis {
        if record(data, *id)?.source_id != trace.source_id {
            return Err(SheetError::Evidence(format!(
                "method record {id:?} belongs to another edition"
            )));
        }
    }
    validate_subjects(data, trace)?;
    for line in &trace.lines {
        crate::linear::validate(&trace.vertices, line)?;
    }
    for unit in &trace.units {
        if let Some(span) = &unit.validity {
            span.validate()
                .map_err(|error| SheetError::Evidence(error.to_string()))?;
        }
        resolve(&trace.vertices, &unit.polygons).map_err(|source| SheetError::Topology {
            record_id: unit.record_id,
            source,
        })?;
    }
    Ok(())
}

fn validate_subjects(data: &Dataset, trace: &SheetTrace) -> Result<(), SheetError> {
    let mut body = None;
    for ids in [
        trace.units.iter().map(|u| u.record_id).collect::<Vec<_>>(),
        trace.places.iter().map(|p| p.record_id).collect(),
        trace.lines.iter().map(|line| line.record_id).collect(),
    ] {
        let mut subjects = BTreeSet::new();
        for id in ids {
            let r = record(data, id)?;
            if r.source_id != trace.source_id || r.scan_page != trace.scan_page {
                return Err(SheetError::Evidence(format!(
                    "{} does not cite the traced sheet",
                    r.key
                )));
            }
            if !subjects.insert(r.entity_id) {
                return Err(SheetError::Evidence(format!(
                    "duplicate sheet subject {:?}",
                    r.entity_id
                )));
            }
            let entity = data
                .entities
                .iter()
                .find(|e| e.id == r.entity_id)
                .ok_or_else(|| SheetError::Evidence(format!("missing entity {:?}", r.entity_id)))?;
            if body.is_some_and(|b| b != entity.body) {
                return Err(SheetError::Evidence(
                    "one sheet cannot mix celestial bodies".into(),
                ));
            }
            body = Some(entity.body);
        }
    }
    Ok(())
}
