//! Independently loadable geometry with bounded static parts.

use atlas_core::identity::EntityId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Full geometry for one evidence-index entry.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryBundle {
    /// Reviewed source raster drawings.
    #[serde(default)]
    pub drawings: Vec<crate::sheet::SheetTrace>,
    /// Registered polygons.
    #[serde(default)]
    pub boundaries: Vec<crate::geographic::GeographicBoundary>,
    /// Terrain-derived polygons.
    #[serde(default)]
    pub derived_boundaries: Vec<crate::terrain::DerivedBoundary>,
    /// Registered open linework.
    #[serde(default)]
    pub lines: Vec<crate::linear::GeographicLine>,
}

/// One byte range of a serialized geometry bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryPart {
    /// Publication-relative static path.
    pub path: String,
    /// Exact byte count.
    pub bytes: usize,
    /// Lowercase SHA-256 digest.
    pub sha256: String,
}

/// Lightweight index entry for loading detail in the current view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryReference {
    /// Ordered UTF-8 JSON byte ranges, assembled before decoding.
    pub parts: Vec<GeometryPart>,
    /// Geographic west, south, east, north extent; absent for pixel-only drawings.
    pub bounds: Option<[f64; 4]>,
    /// Entities whose symbols or outlines occur in the source drawings.
    pub subjects: Vec<EntityId>,
}

impl GeometryReference {
    /// Validate resource paths and allocation bounds before starting requests.
    pub fn validate(&self) -> Result<(), GeometryError> {
        if self.parts.is_empty()
            || self.parts.len() > 128
            || self.parts.iter().any(|part| {
                !part.path.starts_with("geometry/")
                    || !part.path.ends_with(".part")
                    || part
                        .path
                        .split('/')
                        .any(|p| p.is_empty() || p == "." || p == "..")
                    || !part
                        .path
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"/-_.".contains(&c))
                    || part.bytes == 0
                    || part.bytes >= 4_000_000
                    || part.sha256.len() != 64
                    || !part
                        .sha256
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err(GeometryError::Manifest);
        }
        if self.bounds.is_some_and(|[w, s, e, n]| {
            ![w, s, e, n].iter().all(|v| v.is_finite())
                || w > e
                || s > n
                || w < -180.
                || e > 180.
                || s < -90.
                || n > 90.
        }) {
            return Err(GeometryError::Manifest);
        }
        Ok(())
    }

    /// Verify each part and decode the assembled bundle.
    pub fn decode(&self, parts: &[Vec<u8>]) -> Result<GeometryBundle, GeometryError> {
        self.validate()?;
        if parts.len() != self.parts.len() {
            return Err(GeometryError::Manifest);
        }
        let mut bytes = Vec::new();
        for (part, content) in self.parts.iter().zip(parts) {
            if content.len() != part.bytes || fingerprint(content) != part.sha256 {
                return Err(GeometryError::Part(part.path.clone()));
            }
            bytes.extend_from_slice(content);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}

/// SHA-256 used by both geometry publishers and loaders.
pub fn fingerprint(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Invalid geometry manifest, bytes, or JSON.
#[derive(Debug, Error)]
pub enum GeometryError {
    /// Resource paths, bounds, or sizes are inconsistent.
    #[error("invalid geometry manifest")]
    Manifest,
    /// Resource contents differ from their manifest.
    #[error("geometry part failed length or checksum verification: {0}")]
    Part(String),
    /// Geometry could not be decoded.
    #[error("geometry JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests;
