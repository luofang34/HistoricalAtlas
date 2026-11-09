//! Physical tile resources described by the publication asset manifest.

use maplibre::io::{
    resource_loader::SharedLoader,
    source_client::{HttpClient, SourceFetchError},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

mod manifest;
pub use manifest::{Manifest, ManifestError};

#[derive(Clone)]
struct PublishedBasemap(Arc<BTreeMap<String, manifest::Tile>>);

#[async_trait::async_trait(?Send)]
impl HttpClient for PublishedBasemap {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, SourceFetchError> {
        let tile = self
            .0
            .get(url)
            .ok_or_else(|| SourceFetchError::not_found(url))?;
        #[cfg(target_arch = "wasm32")]
        let bytes = crate::resources::bytes(&format!("assets/basemap/{}", tile.file)).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let bytes =
            read_asset_blocking(&tile.file).map_err(|error| SourceFetchError(Box::new(error)))?;
        if format!("{:x}", Sha256::digest(&bytes)) != tile.sha256 {
            return Err(SourceFetchError(Box::new(ManifestError::Invalid(format!(
                "checksum for {}",
                tile.file
            )))));
        }
        Ok(bytes)
    }
}

/// Serve only the tile addresses listed in a validated publication manifest.
pub fn loader(manifest: &Manifest) -> SharedLoader {
    SharedLoader::new(PublishedBasemap(Arc::new(
        manifest
            .tiles
            .iter()
            .map(|tile| {
                (
                    format!("atlas://{}/{}/{}/{}", tile.source, tile.z, tile.x, tile.y),
                    tile.clone(),
                )
            })
            .collect(),
    )))
}

/// Load the publication manifest independently of the executable.
pub async fn load() -> Result<Manifest, SourceFetchError> {
    #[cfg(target_arch = "wasm32")]
    let bytes = crate::resources::bytes("assets/basemap/manifest.json").await?;
    #[cfg(not(target_arch = "wasm32"))]
    let bytes =
        read_asset_blocking("manifest.json").map_err(|error| SourceFetchError(Box::new(error)))?;
    Manifest::parse(&bytes).map_err(|error| SourceFetchError(Box::new(error)))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_asset_blocking(file: &str) -> Result<Vec<u8>, ManifestError> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/basemap")
        .join(file);
    std::fs::read(&path).map_err(|source| ManifestError::Io { path, source })
}

#[cfg(test)]
mod tests;
