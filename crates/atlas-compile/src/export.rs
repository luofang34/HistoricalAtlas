//! UUID-sharded static indexes containing checked records and their citations.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use atlas_core::{assertion::TemporalValue, identity::EntityId};
use atlas_ledger::{
    model::{Dispute, Entity, Estimate, EstimateValue, Record},
    publication::Published,
};
use serde::Serialize;
use thiserror::Error;

use crate::input::LedgerInput;

/// Strict upper bound for every static artifact in bytes.
pub const MAX_FILE_BYTES: usize = 4_000_000;

#[derive(Serialize)]
struct IndexedEntity<'a> {
    entity: &'a Entity,
    records: Vec<&'a Record>,
    estimates: Vec<&'a Estimate>,
    disputes: Vec<&'a Dispute>,
}

/// Named byte buffers ready to write into a publication directory.
pub type Artifacts = BTreeMap<PathBuf, Vec<u8>>;

/// Build deterministic indexes; unchecked observations do not enter the output.
pub fn compile(input: &LedgerInput) -> Result<Artifacts, ExportError> {
    atlas_ledger::validation::validate(&input.catalog, &input.data)?;
    atlas_ledger::publication::verify_publication(&input.data, &input.aliases, &input.published)?;
    crate::plate_models::validate_models(&input.plate_models, &input.catalog, &input.data)?;
    let shards = index_entities(input);
    let current: BTreeSet<_> = shards
        .values()
        .flat_map(|shard| shard.keys().copied())
        .collect();
    verify_checked_references(input, &current)?;
    let mut output = BTreeMap::new();
    let shard_names: Vec<_> = shards.keys().cloned().collect();
    for (shard, entities) in shards {
        insert(
            &mut output,
            &format!("entities/index/{shard}.json"),
            &entities,
        )?;
    }
    insert(&mut output, "entities/shards.json", &shard_names)?;
    insert(&mut output, "entities/aliases.json", &input.aliases)?;
    let published = Published(input.published.0.union(&current).copied().collect());
    insert(&mut output, "entities/published.json", &published)?;
    let mut sources: Vec<_> = input.catalog.sources.iter().collect();
    sources.sort_by(|a, b| a.id.cmp(&b.id));
    insert(&mut output, "sources.json", &sources)?;
    let models: Vec<_> = input.plate_models.keys().collect();
    insert(&mut output, "plates/models.json", &models)?;
    for (id, model) in &input.plate_models {
        insert(&mut output, &format!("plates/{id}.json"), model)?;
    }
    Ok(output)
}

fn index_entities(input: &LedgerInput) -> BTreeMap<String, BTreeMap<EntityId, IndexedEntity<'_>>> {
    let mut entities: BTreeMap<_, _> = input
        .data
        .entities
        .iter()
        .map(|entity| {
            (
                entity.id,
                IndexedEntity {
                    entity,
                    records: Vec::new(),
                    estimates: Vec::new(),
                    disputes: Vec::new(),
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
    let mut shards: BTreeMap<String, BTreeMap<EntityId, IndexedEntity<'_>>> = BTreeMap::new();
    for (id, mut index) in entities {
        if index.records.is_empty() {
            continue;
        }
        index.records.sort_by_key(|r| r.id);
        index.estimates.sort_by_key(|e| e.id);
        index.disputes.sort_by_key(|d| d.estimate_id);
        shards.entry(id.shard()).or_default().insert(id, index);
    }
    shards
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
