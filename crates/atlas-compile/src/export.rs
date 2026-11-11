//! UUID-sharded static indexes containing checked records and their citations.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use atlas_boundary::geographic::GeographicBoundary;
use atlas_core::{assertion::TemporalValue, identity::EntityId};
use atlas_ledger::model::{Dispute, Entity, Estimate, EstimateValue, Record};
use serde::Serialize;
use thiserror::Error;

use crate::input::LedgerInput;

pub(crate) mod chunks;
pub(crate) mod geometry;
mod reader;
pub(crate) mod sharding;
mod storage;

pub use reader::{entity_shard_paths, load_sources, lookup_entity, read_manifest};

/// Strict upper bound for every static artifact in bytes.
pub const MAX_FILE_BYTES: usize = 4_000_000;

#[derive(Serialize)]
pub(crate) struct IndexedEntity<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    geometry: Option<atlas_boundary::publication::GeometryReference>,
    entity: &'a Entity,
    records: Vec<&'a Record>,
    estimates: Vec<&'a Estimate>,
    disputes: Vec<&'a Dispute>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    drawings: Vec<&'a atlas_boundary::sheet::SheetTrace>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    boundaries: Vec<&'a GeographicBoundary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lines: Vec<&'a atlas_boundary::linear::GeographicLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    derived_boundaries: Vec<&'a atlas_boundary::terrain::DerivedBoundary>,
}

/// Named byte buffers ready to write into a publication directory.
pub type Artifacts = BTreeMap<PathBuf, Vec<u8>>;

/// Resolve one entity's geometry parts, retaining support for inline publication geometry.
pub fn resolve_geometry(
    output: &Artifacts,
    entry: &serde_json::Value,
) -> Result<serde_json::Value, ExportError> {
    storage::resolve(output, entry)
}

/// Build deterministic indexes; unchecked observations do not enter the output.
pub fn compile(input: &LedgerInput) -> Result<Artifacts, ExportError> {
    let mut derived = Vec::new();
    for request in input.derivations.values() {
        derived.extend(
            crate::derivation::reconstruct(input, request)
                .map_err(|e| ExportError::Derivation(Box::new(e)))?,
        );
    }
    let mut artifacts = compile_with_derived(input, &derived)?;
    for request in input.derivations.values() {
        let recipe = crate::derivation::published(input, request)
            .map_err(|e| ExportError::Derivation(Box::new(e)))?;
        insert(
            &mut artifacts,
            &format!("derived/{}.json", request.id),
            &recipe,
        )?;
    }
    Ok(artifacts)
}

#[cfg(test)]
pub(crate) fn compile_with_boundaries(
    input: &LedgerInput,
    boundaries: &[GeographicBoundary],
) -> Result<Artifacts, ExportError> {
    compile_geometry(
        input,
        boundaries,
        &[],
        &[],
        sharding::DEFAULT_TARGET_SHARD_BYTES,
    )
}

#[cfg(test)]
pub(crate) fn compile_with_target_bytes(
    input: &LedgerInput,
    target_bytes: usize,
) -> Result<Artifacts, ExportError> {
    compile_geometry(input, &[], &[], &[], target_bytes)
}

pub(crate) fn compile_with_derived(
    input: &LedgerInput,
    derived: &[atlas_boundary::terrain::DerivedBoundary],
) -> Result<Artifacts, ExportError> {
    compile_geometry(
        input,
        &[],
        derived,
        &[],
        sharding::DEFAULT_TARGET_SHARD_BYTES,
    )
}

pub(crate) fn compile_registered(
    input: &LedgerInput,
    boundaries: &[GeographicBoundary],
    lines: &[atlas_boundary::linear::GeographicLine],
) -> Result<Artifacts, ExportError> {
    compile_geometry(
        input,
        boundaries,
        &[],
        lines,
        sharding::DEFAULT_TARGET_SHARD_BYTES,
    )
}

fn compile_geometry(
    input: &LedgerInput,
    boundaries: &[GeographicBoundary],
    derived: &[atlas_boundary::terrain::DerivedBoundary],
    explicit_lines: &[atlas_boundary::linear::GeographicLine],
    target_shard_bytes: usize,
) -> Result<Artifacts, ExportError> {
    atlas_ledger::validation::validate(&input.catalog, &input.data)?;
    atlas_ledger::publication::verify_publication(&input.data, &input.aliases, &input.published)?;
    crate::plate_models::validate_models(&input.plate_models, &input.catalog, &input.data)?;
    for trace in &input.traces {
        atlas_boundary::sheet::validate(&input.catalog, &input.data, trace)?;
    }
    let shapes = crate::place_maps::register_shapes(input)
        .map_err(|source| ExportError::PlaceMap(Box::new(source)))?;
    let mut registered = shapes.boundaries;
    let mut lines = shapes.lines;
    registered.retain(|b| {
        !boundaries
            .iter()
            .any(|other| b.unit.record_id == other.unit.record_id)
    });
    registered.extend_from_slice(boundaries);
    let boundaries = registered.as_slice();
    for boundary in boundaries {
        boundary.feature(&input.data, None)?;
    }
    for boundary in derived {
        boundary.feature(&input.data, None)?;
    }
    lines.retain(|line| {
        !explicit_lines
            .iter()
            .any(|other| line.line.record_id == other.line.record_id)
    });
    lines.extend_from_slice(explicit_lines);
    for line in &lines {
        line.feature(&input.data, None)?;
    }
    let mut entities = index_entities(input, boundaries, derived, &lines);
    let current: BTreeSet<_> = entities.keys().copied().collect();
    verify_checked_references(input, &current)?;
    let (geometry_spans, cross_cited) =
        sharding::summary::collect_metadata(&entities, input, derived);
    let mut output = BTreeMap::new();
    storage::detach(&mut output, &mut entities, &input.data)?;
    let (shard_summaries, catalog_shards) = sharding::publish(
        &mut output,
        &entities,
        &geometry_spans,
        &cross_cited,
        target_shard_bytes,
    )?;
    reader::publish_manifest(
        &mut output,
        input,
        &current,
        shard_summaries,
        catalog_shards,
    )?;
    geometry::publish_maps(&mut output, input, &shapes.maps)?;
    Ok(output)
}

fn index_entities<'a>(
    input: &'a LedgerInput,
    boundaries: &'a [GeographicBoundary],
    derived: &'a [atlas_boundary::terrain::DerivedBoundary],
    lines: &'a [atlas_boundary::linear::GeographicLine],
) -> BTreeMap<EntityId, IndexedEntity<'a>> {
    let mut entities: BTreeMap<_, _> = input
        .data
        .entities
        .iter()
        .map(|entity| {
            (
                entity.id,
                IndexedEntity {
                    geometry: None,
                    entity,
                    records: Vec::new(),
                    estimates: Vec::new(),
                    disputes: Vec::new(),
                    drawings: Vec::new(),
                    boundaries: Vec::new(),
                    lines: Vec::new(),
                    derived_boundaries: Vec::new(),
                },
            )
        })
        .collect();
    for record in input.data.records.iter().filter(|r| r.checked) {
        if let Some(index) = entities.get_mut(&record.entity_id) {
            index.records.push(record);
        }
    }
    for estimate in &input.data.estimates {
        if let Some(index) = entities.get_mut(&estimate.entity_id) {
            index.estimates.push(estimate);
        }
    }
    for dispute in &input.data.disputes {
        if let Some(index) = entities.get_mut(&dispute.entity_id) {
            index.disputes.push(dispute);
        }
    }
    geometry::index(input, &mut entities, boundaries, derived, lines);
    entities.retain(|_, index| !index.records.is_empty());
    for index in entities.values_mut() {
        index.records.sort_by_key(|r| r.id);
        index.estimates.sort_by_key(|e| e.id);
        index.disputes.sort_by_key(|d| d.estimate_id);
    }
    entities
}

fn verify_checked_references(
    input: &LedgerInput,
    current: &BTreeSet<EntityId>,
) -> Result<(), ExportError> {
    for id in input.published.0.iter().chain(input.aliases.0.keys()) {
        let resolved = input.aliases.resolve(*id)?;
        if !current.contains(&resolved) {
            return Err(ExportError::MissingEvidence(*id));
        }
    }
    for record in input.data.records.iter().filter(|r| r.checked) {
        for target in record
            .observation
            .parent_id
            .iter()
            .chain(&record.observation.seat_id)
            .chain(
                record
                    .observation
                    .physical
                    .iter()
                    .filter_map(|physical| physical.flows_into.as_ref()),
            )
            .chain(
                record
                    .observation
                    .territory
                    .iter()
                    .flat_map(|t| &t.neighbors)
                    .filter_map(|n| n.entity_id.as_ref()),
            )
        {
            if !current.contains(target) {
                return Err(ExportError::MissingEvidence(*target));
            }
        }
    }
    for estimate in &input.data.estimates {
        if let EstimateValue::Temporal { assertion } = &estimate.value
            && let TemporalValue::Hierarchy(hierarchy) = &assertion.value
        {
            for target in hierarchy.parent_id.iter().chain(&hierarchy.seat_id) {
                if !current.contains(target) {
                    return Err(ExportError::MissingEvidence(*target));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn insert(
    output: &mut Artifacts,
    path: &str,
    value: &impl Serialize,
) -> Result<(), ExportError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    insert_bytes(output, path, bytes)
}

pub(crate) fn insert_bytes(
    output: &mut Artifacts,
    path: &str,
    bytes: Vec<u8>,
) -> Result<(), ExportError> {
    if bytes.len() >= MAX_FILE_BYTES {
        return Err(ExportError::Oversize {
            path: path.into(),
            bytes: bytes.len(),
        });
    }
    output.insert(PathBuf::from(path), bytes);
    Ok(())
}

/// Write into a new directory, avoiding stale artifacts from an earlier build.
pub fn write_blocking(output: &Path, artifacts: &Artifacts) -> Result<(), ExportError> {
    fs::create_dir(output).map_err(|source| ExportError::Io {
        path: output.to_owned(),
        source,
    })?;
    for (relative, bytes) in artifacts {
        let path = output.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ExportError::Io {
                path: parent.to_owned(),
                source,
            })?;
        }
        fs::write(&path, bytes).map_err(|source| ExportError::Io { path, source })?;
    }
    Ok(())
}

/// Static artifact construction or write failure.
#[derive(Debug, Error)]
pub enum ExportError {
    /// Publication manifest or verified part failed validation.
    #[error(transparent)]
    Manifest(#[from] atlas_boundary::publication::ManifestError),
    /// External geometry manifest or data failed validation.
    #[error(transparent)]
    Geometry(#[from] atlas_boundary::publication::GeometryError),
    /// Registered linework or its source evidence is invalid.
    #[error(transparent)]
    Line(#[from] atlas_boundary::linear::LineError),
    /// A terrain recipe failed source resolution or reconstruction.
    #[error("terrain reconstruction: {0}")]
    Derivation(#[source] Box<crate::derivation::DerivationError>),
    /// Terrain reconstruction failed evidence or topology validation.
    #[error(transparent)]
    Terrain(#[from] atlas_boundary::terrain::TerrainError),
    /// Registered geographic boundary or its evidence is invalid.
    #[error(transparent)]
    Boundary(#[from] atlas_boundary::geographic::GeographicError),
    /// Reviewed source-sheet geometry or evidence failed validation.
    #[error(transparent)]
    Sheet(#[from] atlas_boundary::sheet::SheetError),
    /// A source map failed evidence or registration checks.
    #[error("place map export: {0}")]
    PlaceMap(#[source] Box<crate::place_maps::PlaceMapError>),
    /// Invalid model or missing plate-assignment evidence.
    #[error("plate model export: {0}")]
    Plate(#[from] crate::plate_models::PlateDocumentError),
    /// JSON serialization failed.
    #[error("index serialization: {0}")]
    Json(#[from] serde_json::Error),
    /// A file exceeded the static publication budget.
    #[error("{path} is {bytes} bytes; every file must be smaller than {MAX_FILE_BYTES}")]
    Oversize {
        /// Artifact path.
        path: String,
        /// Serialized size.
        bytes: usize,
    },
    /// Published entity has no checked evidence.
    #[error("referenced entity {0:?} has no checked evidence")]
    MissingEvidence(EntityId),
    /// Ledger integrity failure.
    #[error("ledger validation: {0}")]
    Validation(#[from] atlas_ledger::validation::ValidationError),
    /// Directory creation or write error.
    #[error("cannot write {path}: {source}")]
    Io {
        /// Artifact path.
        path: PathBuf,
        /// Underlying operating-system failure.
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests;
