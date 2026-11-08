//! Filesystem input with errors identifying the exact ledger document.

use std::{
    fs,
    path::{Path, PathBuf},
};

use atlas_ledger::{
    model::{Dataset, SourceCatalog},
    publication::{Aliases, Published, verify_publication},
    validation::{ValidationError, validate},
};
use serde::de::DeserializeOwned;
use thiserror::Error;

/// Validated input bundle including the existing publication contract.
#[derive(Debug)]
pub struct LedgerInput {
    /// Combined source catalog.
    pub catalog: SourceCatalog,
    /// Combined geographical batches.
    pub data: Dataset,
    /// Published redirects.
    pub aliases: Aliases,
    /// IDs already promised to consumers.
    pub published: Published,
}

/// File loading or ledger integrity failure.
#[derive(Debug, Error)]
pub enum InputError {
    /// Read or directory traversal failure.
    #[error("cannot read {path}: {source}")]
    Io {
        /// Input path.
        path: PathBuf,
        /// Underlying operating-system error.
        #[source]
        source: std::io::Error,
    },
    /// Malformed JSON.
    #[error("invalid JSON in {path}: {source}")]
    Json {
        /// Input path.
        path: PathBuf,
        /// Parser error.
        #[source]
        source: serde_json::Error,
    },
    /// Malformed source catalog.
    #[error("invalid source catalog {path}: {source}")]
    Toml {
        /// Input path.
        path: PathBuf,
        /// Parser error.
        #[source]
        source: toml::de::Error,
    },
    /// Ledger consistency failure.
    #[error("ledger validation: {0}")]
    Validation(#[from] ValidationError),
    /// No source or evidence documents were supplied.
    #[error("{0} contains no source catalog or no checked records")]
    Empty(PathBuf),
}

/// Read all `sources.toml` and `*.dataset.json` batches below a data directory.
pub fn load_blocking(root: &Path) -> Result<LedgerInput, InputError> {
    let mut paths = Vec::new();
    collect_blocking(root, &mut paths)?;
    paths.sort();
    let mut catalog = SourceCatalog {
        sources: Vec::new(),
    };
    let mut data = Dataset::default();
    for path in paths {
        if path.file_name().is_some_and(|name| name == "sources.toml") {
            let content = read_blocking(&path)?;
            let mut batch: SourceCatalog =
                toml::from_str(&content).map_err(|source| InputError::Toml {
                    path: path.clone(),
                    source,
                })?;
            catalog.sources.append(&mut batch.sources);
        } else if path.to_string_lossy().ends_with(".dataset.json") {
            let mut batch: Dataset = read_json_blocking(&path)?;
            data.entities.append(&mut batch.entities);
            data.records.append(&mut batch.records);
            data.estimates.append(&mut batch.estimates);
            data.disputes.append(&mut batch.disputes);
        }
    }
    if catalog.sources.is_empty() || !data.records.iter().any(|record| record.checked) {
        return Err(InputError::Empty(root.to_owned()));
    }
    validate(&catalog, &data)?;
    let aliases = read_json_blocking(&root.join("aliases.json"))?;
    let published = read_json_blocking(&root.join("published.json"))?;
    verify_publication(&data, &aliases, &published)?;
    Ok(LedgerInput {
        catalog,
        data,
        aliases,
        published,
    })
}

fn read_blocking(path: &Path) -> Result<String, InputError> {
    fs::read_to_string(path).map_err(|source| InputError::Io {
        path: path.to_owned(),
        source,
    })
}

fn read_json_blocking<T: DeserializeOwned>(path: &Path) -> Result<T, InputError> {
    serde_json::from_str(&read_blocking(path)?).map_err(|source| InputError::Json {
        path: path.to_owned(),
        source,
    })
}

fn collect_blocking(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), InputError> {
    let entries = fs::read_dir(root).map_err(|source| InputError::Io {
        path: root.to_owned(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| InputError::Io {
            path: root.to_owned(),
            source,
        })?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|source| InputError::Io {
            path: path.clone(),
            source,
        })?;
        if kind.is_dir() {
            collect_blocking(&path, paths)?;
        }
        if kind.is_file() {
            paths.push(path);
        }
    }
    Ok(())
}
