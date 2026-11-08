//! Read the compiler's published shards from a local directory.

use serde::de::DeserializeOwned;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::{
    catalog::{Atlas, Entry, Publication},
    error::AppError,
};

/// Filesystem publication loading failure.
#[derive(Debug, Error)]
pub enum PublicationError {
    /// A file could not be read.
    #[error("cannot read {path}: {source}")]
    Io {
        /// Requested file.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// A document could not be decoded.
    #[error("invalid publication document {path}: {source}")]
    Json {
        /// Requested file.
        path: PathBuf,
        /// Underlying JSON error.
        #[source]
        source: serde_json::Error,
    },
    /// Shard names must be two lowercase hexadecimal digits.
    #[error("invalid entity shard name {0:?}")]
    Shard(String),
    /// Entity identity was repeated in more than one shard.
    #[error("duplicate entity in shard {0}")]
    Duplicate(String),
    /// Loaded evidence failed integrity checks.
    #[error(transparent)]
    Atlas(#[from] AppError),
}

/// Load exactly the shards named by the publication manifest.
pub fn load_blocking(root: &Path) -> Result<Atlas, PublicationError> {
    let sources = read_blocking(&root.join("sources.json"))?;
    let aliases = read_blocking(&root.join("entities/aliases.json"))?;
    let shards: Vec<String> = read_blocking(&root.join("entities/shards.json"))?;
    let mut entries = BTreeMap::new();
    for shard in shards {
        if shard.len() != 2
            || !shard
                .bytes()
                .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        {
            return Err(PublicationError::Shard(shard));
        }
        let records: BTreeMap<_, Entry> =
            read_blocking(&root.join(format!("entities/index/{shard}.json")))?;
        for (id, record) in records {
            if entries.insert(id, record).is_some() {
                return Err(PublicationError::Duplicate(shard));
            }
        }
    }
    Ok(Atlas::new(Publication {
        sources,
        entries,
        aliases,
    })?)
}

fn read_blocking<T: DeserializeOwned>(path: &Path) -> Result<T, PublicationError> {
    let bytes = fs::read(path).map_err(|source| PublicationError::Io {
        path: path.to_owned(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| PublicationError::Json {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests;
