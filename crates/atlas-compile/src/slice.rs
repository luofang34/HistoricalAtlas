//! Static time slices with UUID shards and a complete evidence index.

use std::{collections::BTreeMap, num::ParseIntError, path::Path};

use atlas_core::time::{EpochTime, TimeError};
use atlas_ledger::{
    timeline::{SelectionError, TimeSelection, Timeline},
    validation::ValidationError,
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    export::{Artifacts, ExportError, compile, insert, write_blocking},
    input::{InputError, LedgerInput, load_blocking},
};

#[derive(Serialize)]
struct SliceManifest {
    at: EpochTime,
    selection: TimeSelection,
    shards: Vec<String>,
}

/// Compile time-selected properties alongside the complete checked evidence index.
pub fn compile_slice(
    input: &LedgerInput,
    at: EpochTime,
    selection: TimeSelection,
) -> Result<Artifacts, SliceError> {
    let mut artifacts = compile(input)?;
    let timeline = Timeline::new(&input.catalog, &input.data)?;
    let slice = timeline.at(at, selection)?;
    let mut shards: BTreeMap<String, BTreeMap<_, _>> = BTreeMap::new();
    for entity in slice.entities {
        shards
            .entry(entity.entity_id.shard())
            .or_default()
            .insert(entity.entity_id, entity);
    }
    let names = shards.keys().cloned().collect();
    for (name, entities) in shards {
        insert(
            &mut artifacts,
            &format!("slice/index/{name}.json"),
            &entities,
        )?;
    }
    insert(
        &mut artifacts,
        "slice.json",
        &SliceManifest {
            at,
            selection,
            shards: names,
        },
    )?;
    Ok(artifacts)
}

/// Parse a time query, load its ledger, and write artifacts into a new directory.
pub fn write_slice_blocking(
    root: &Path,
    ticks: &str,
    selection: &str,
    output: &Path,
) -> Result<usize, SliceError> {
    let at = EpochTime::try_from(ticks.to_owned()).map_err(|source| SliceError::Ticks {
        value: ticks.to_owned(),
        source,
    })?;
    let selection = selection.parse()?;
    let input = load_blocking(root)?;
    let artifacts = compile_slice(&input, at, selection)?;
    write_blocking(output, &artifacts)?;
    Ok(artifacts.len())
}

/// Invalid query, ledger, or artifact output.
#[derive(Debug, Error)]
pub enum SliceError {
    /// Millisecond coordinate could not be parsed exactly.
    #[error("invalid TT millisecond coordinate {value:?}: {source}")]
    Ticks {
        /// Supplied coordinate string.
        value: String,
        /// Integer parser failure.
        #[source]
        source: ParseIntError,
    },
    /// Unknown selection mode.
    #[error(transparent)]
    Selection(#[from] SelectionError),
    /// Ledger loading failed.
    #[error("time-slice input: {0}")]
    Input(#[from] InputError),
    /// Ledger integrity failure.
    #[error("time-slice ledger: {0}")]
    Validation(#[from] ValidationError),
    /// Temporal interval failed validation.
    #[error("time-slice interval: {0}")]
    Time(#[from] TimeError),
    /// Static artifact generation failed.
    #[error("time-slice output: {0}")]
    Export(#[from] ExportError),
}

#[cfg(test)]
mod tests;
