//! Physical tile resources described by the publication asset manifest.

use maplibre::io::{
    resource_loader::SharedLoader,
    source_client::{HttpClient, SourceFetchError},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

mod manifest;
pub use manifest::{Manifest, ManifestError};

#[cfg(not(target_arch = "wasm32"))]
type Network = maplibre::platform::http_client::ReqwestHttpClient;
#[cfg(target_arch = "wasm32")]
type Network = maplibre::platform::http_client::WHATWGFetchHttpClient;

/// Bundled tiles by address, plus the online services the manifest declares.
#[derive(Clone)]
struct PublishedBasemap {
    tiles: Arc<BTreeMap<String, manifest::Tile>>,
    remote: Arc<Vec<manifest::RemoteSource>>,
    network: Network,
}

#[cfg(target_arch = "wasm32")]
fn resource_to_fetch(error: crate::resources::ResourceError) -> SourceFetchError {
    match error {
        crate::resources::ResourceError::NotFound(url) => SourceFetchError::not_found(&url),
        crate::resources::ResourceError::Http {
            url,
            status,
            reason,
        } => SourceFetchError::http_response(
            &url,
            status,
            ManifestError::Invalid(format!("HTTP {status}: {reason}")),
        ),
        other => SourceFetchError::temporary(other),
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl HttpClient for PublishedBasemap {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, SourceFetchError> {
        if self.remote.iter().any(|source| source.serves(url)) {
            return self.network.fetch(url).await;
        }
        let tile = self
            .tiles
            .get(url)
            .ok_or_else(|| SourceFetchError::not_found(url))?;
        #[cfg(target_arch = "wasm32")]
        let bytes = crate::resources::bytes(&format!("assets/basemap/{}", tile.file))
            .await
            .map_err(resource_to_fetch)?;
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

/// Serve the bundled tiles a validated manifest lists and fetch only its declared services.
pub fn loader(manifest: &Manifest) -> SharedLoader {
    SharedLoader::new(PublishedBasemap {
        tiles: Arc::new(
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
        ),
        remote: Arc::new(manifest.remote.clone()),
        #[cfg(not(target_arch = "wasm32"))]
        network: Network::new(tile_cache()),
        #[cfg(target_arch = "wasm32")]
        network: Network::default(),
    })
}

/// Load the publication manifest independently of the executable.
pub async fn load() -> Result<Manifest, ManifestError> {
    #[cfg(target_arch = "wasm32")]
    let bytes = crate::resources::bytes("assets/basemap/manifest.json").await?;
    #[cfg(not(target_arch = "wasm32"))]
    let bytes = read_asset_blocking("manifest.json")?;
    Manifest::parse(&bytes)
}

/// Platform cache directory for online tiles; HTTP caching headers decide reuse,
/// and without a cache directory tiles are fetched again on every launch.
#[cfg(not(target_arch = "wasm32"))]
fn tile_cache() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|directory| directory.join("historical-atlas").join("tiles"))
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
