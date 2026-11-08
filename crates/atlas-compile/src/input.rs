//! Filesystem input with errors identifying the exact ledger document.

use std::{
    collections::BTreeMap,
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

use crate::plate_models::{PlateDocumentError, PlateModelDocument, validate_models};

/// Validated input bundle including the existing publication contract.
#[derive(Debug)]
pub struct LedgerInput {
    /// Registered source maps supplying derived place locations.
    pub place_maps: BTreeMap<String, crate::place_maps::PlaceMap>,
    /// Cited plate models available for reconstruction.
    pub plate_models: BTreeMap<String, PlateModelDocument>,
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
    /// A source map could not supply validated place estimates.
    #[error("invalid place map {path}: {source}")]
    PlaceMap {
        /// Map document path.
        path: PathBuf,
        /// Underlying evidence or registration failure.
        #[source]
        source: Box<crate::place_maps::PlaceMapError>,
    },
    /// A source trace failed evidence or polygon validation.
    #[error("invalid map trace {path}: {source}")]
    Trace {
        /// Trace document path.
        path: PathBuf,
        /// Underlying evidence or geometry failure.
        #[source]
        source: Box<crate::boundary::BoundaryError>,
    },
    /// Rotation model metadata or assignment failed validation.
    #[error("plate model input: {0}")]
    Plate(#[from] PlateDocumentError),
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

/// Read source catalogs, evidence batches, plate models, and source maps.
pub fn load_blocking(root: &Path) -> Result<LedgerInput, InputError> {
    let mut paths = Vec::new();
    collect_blocking(root, &mut paths)?;
    paths.sort();
    let mut catalog = SourceCatalog {
        sources: Vec::new(),
    };
    let mut data = Dataset::default();
    let mut plate_models = BTreeMap::new();
    let mut traces = Vec::new();
    let mut place_maps = Vec::new();
    for path in paths {
        if path.file_name().is_some_and(|name| name == "sources.toml") {
            let mut batch = read_catalog_blocking(&path)?;
            catalog.sources.append(&mut batch.sources);
        } else if path.to_string_lossy().ends_with(".plates.json") {
            let document: PlateModelDocument = read_json_blocking(&path)?;
            let id = document.id.clone();
            if plate_models.insert(id.clone(), document).is_some() {
                return Err(PlateDocumentError::Invalid {
                    model: id,
                    reason: "duplicate model document".into(),
                }
                .into());
            }
        } else if path.to_string_lossy().ends_with(".dataset.json") {
            let mut batch: Dataset = read_json_blocking(&path)?;
            data.entities.append(&mut batch.entities);
            data.records.append(&mut batch.records);
            data.estimates.append(&mut batch.estimates);
            data.disputes.append(&mut batch.disputes);
        } else if path.to_string_lossy().ends_with(".trace.json") {
            traces.push((path.clone(), read_json_blocking(&path)?));
        } else if path.to_string_lossy().ends_with(".places.json") {
            place_maps.push((path.clone(), read_json_blocking(&path)?));
        }
    }
    if catalog.sources.is_empty() || !data.records.iter().any(|record| record.checked) {
        return Err(InputError::Empty(root.to_owned()));
    }
    validate(&catalog, &data)?;
    validate_models(&plate_models, &catalog, &data)?;
    let aliases = read_json_blocking(&root.join("aliases.json"))?;
    let published = read_json_blocking(&root.join("published.json"))?;
    verify_publication(&data, &aliases, &published)?;
    let mut input = LedgerInput {
        place_maps: BTreeMap::new(),
        plate_models,
        catalog,
        data,
        aliases,
        published,
    };
    load_place_maps(&mut input, place_maps)?;
    for (path, trace) in traces {
        crate::boundary::validate_trace(&input, &trace).map_err(|source| InputError::Trace {
            path,
            source: Box::new(source),
        })?;
    }
    Ok(input)
}

fn read_catalog_blocking(path: &Path) -> Result<SourceCatalog, InputError> {
    toml::from_str(&read_blocking(path)?).map_err(|source| InputError::Toml {
        path: path.to_owned(),
        source,
    })
}

fn load_place_maps(
    input: &mut LedgerInput,
    maps: Vec<(PathBuf, crate::place_maps::PlaceMap)>,
) -> Result<(), InputError> {
    for (path, map) in maps {
        let (_, estimates) = crate::place_maps::register_places(input, &map).map_err(|source| {
            InputError::PlaceMap {
                path: path.clone(),
                source: Box::new(source),
            }
        })?;
        if input.place_maps.contains_key(&map.id) {
            return Err(InputError::PlaceMap {
                path,
                source: Box::new(crate::place_maps::PlaceMapError::Evidence(format!(
                    "duplicate map {}",
                    map.id
                ))),
            });
        }
        input.data.estimates.extend(estimates);
        input.place_maps.insert(map.id.clone(), map);
    }
    validate(&input.catalog, &input.data)?;
    Ok(())
}

fn read_blocking(path: &Path) -> Result<String, InputError> {
    fs::read_to_string(path).map_err(|source| InputError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn read_json_blocking<T: DeserializeOwned>(path: &Path) -> Result<T, InputError> {
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
