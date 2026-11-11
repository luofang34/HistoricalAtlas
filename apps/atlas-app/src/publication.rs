//! Read and verify the compiler's versioned publication shards from a local directory.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

use atlas_boundary::publication::{
    CatalogItem, ManifestError, PublicationManifest, ShardSummary, VerifiedPart,
    decode_concatenated, decode_map_parts, decode_vec_parts,
};
use atlas_ledger::publication::{Aliases, Published};

use crate::{
    catalog::{Atlas, Bootstrap},
    error::AppError,
};

/// Filesystem publication loading failure.
#[derive(Debug, Error)]
pub enum PublicationError {
    /// Root manifest or part checksum failed verification.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// Geometry parts failed verification.
    #[error(transparent)]
    Geometry(#[from] atlas_boundary::publication::GeometryError),
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
    /// Loaded evidence failed integrity checks.
    #[error(transparent)]
    Atlas(#[from] AppError),
}

/// Load and verify the compact startup manifest, bibliography, search catalog, and plate models.
pub fn load_bootstrap_blocking(root: &Path) -> Result<Bootstrap, PublicationError> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = fs::read(&manifest_path).map_err(|source| PublicationError::Io {
        path: manifest_path.clone(),
        source,
    })?;
    let manifest: PublicationManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PublicationError::Json {
            path: manifest_path,
            source,
        })?;
    manifest.validate()?;
    let sources = decode_vec_parts(
        &manifest.sources,
        &read_parts_blocking(root, &manifest.sources)?,
    )?;
    let aliases = Aliases(decode_map_parts(
        &manifest.aliases,
        &read_parts_blocking(root, &manifest.aliases)?,
    )?);
    let published = Published(
        decode_vec_parts(
            &manifest.published,
            &read_parts_blocking(root, &manifest.published)?,
        )?
        .into_iter()
        .collect(),
    );
    let shards: Vec<ShardSummary> = decode_vec_parts(
        &manifest.shard_inventories,
        &read_parts_blocking(root, &manifest.shard_inventories)?,
    )?;
    let catalog_parts: Vec<VerifiedPart> = manifest
        .catalog_shards
        .iter()
        .map(|s| s.part.clone())
        .collect();
    let catalog: Vec<CatalogItem> =
        decode_vec_parts(&catalog_parts, &read_parts_blocking(root, &catalog_parts)?)?;
    let mut plate_models = BTreeMap::new();
    for model in &manifest.plate_models {
        let slices = read_parts_blocking(root, &model.parts)?;
        let document = decode_concatenated(&model.parts, &slices)?;
        plate_models.insert(model.id.clone(), document);
    }
    Ok(Bootstrap {
        manifest,
        sources,
        aliases,
        published,
        shards,
        catalog,
        plate_models,
    })
}

/// Load and verify all shards and geometry bundles named by `manifest.json`.
pub fn load_blocking(root: &Path) -> Result<Atlas, PublicationError> {
    let bootstrap = load_bootstrap_blocking(root)?;
    let mut atlas = Atlas::from_bootstrap(bootstrap)?;
    let ordered_shards: Vec<ShardSummary> = atlas.ordered_shards()?.into_iter().cloned().collect();
    let mut geometries = Vec::new();
    for shard in &ordered_shards {
        let path = root.join(&shard.part.path);
        let bytes = fs::read(&path).map_err(|source| PublicationError::Io { path, source })?;
        let attached = atlas.attach_shard(&shard.id, &bytes)?;
        geometries.extend(attached.geometries);
    }
    for (id, _, reference) in geometries {
        reference.validate()?;
        let parts = reference
            .parts
            .iter()
            .map(|part| {
                let path = root.join(&part.path);
                fs::read(&path).map_err(|source| PublicationError::Io { path, source })
            })
            .collect::<Result<Vec<_>, _>>()?;
        atlas.attach_geometry(id, reference.decode(&parts)?)?;
    }
    Ok(atlas)
}

fn read_parts_blocking(
    root: &Path,
    parts: &[VerifiedPart],
) -> Result<Vec<Vec<u8>>, PublicationError> {
    parts
        .iter()
        .map(|part| {
            part.validate_with_prefix("")?;
            let path = root.join(&part.path);
            fs::read(&path).map_err(|source| PublicationError::Io { path, source })
        })
        .collect()
}

#[cfg(test)]
mod tests;
