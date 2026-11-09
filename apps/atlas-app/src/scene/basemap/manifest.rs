//! Tile-address validation and body-specific raster style construction.

use atlas_geo::body::BodyId;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Invalid publication asset configuration.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// Manifest JSON cannot be decoded.
    #[error("basemap manifest: {0}")]
    Json(#[from] serde_json::Error),
    /// A tile address, path, or fingerprint is invalid.
    #[error("invalid basemap manifest: {0}")]
    Invalid(String),
    /// A native asset cannot be read.
    #[error("basemap asset {}: {source}", path.display())]
    Io {
        /// File requested by the manifest.
        path: std::path::PathBuf,
        /// Filesystem failure.
        #[source]
        source: std::io::Error,
    },
}

/// Physical imagery inventory; tile addresses determine its zooms and coverage.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    /// Attribution shown with physical imagery.
    pub attribution: String,
    pub(super) tiles: Vec<Tile>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Tile {
    pub source: String,
    pub z: u8,
    pub x: u32,
    pub y: u32,
    pub file: String,
    pub sha256: String,
    #[serde(default = "earth")]
    pub body: BodyId,
}

fn earth() -> BodyId {
    BodyId::Earth
}

impl Manifest {
    /// Decode and validate paths, addresses, fingerprints, and source body consistency.
    pub fn parse(bytes: &[u8]) -> Result<Self, ManifestError> {
        let result: Self = serde_json::from_slice(bytes)?;
        let mut addresses = BTreeSet::new();
        let mut bodies = BTreeMap::new();
        for tile in &result.tiles {
            if tile.source.is_empty()
                || !tile
                    .source
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
                || tile.z > 22
                || tile.x >= (1 << tile.z)
                || tile.y >= (1 << tile.z)
                || tile.file.is_empty()
                || !tile
                    .file
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                || !tile.file.ends_with(".jpg")
                || tile.file.contains("..")
                || tile.sha256.len() != 64
                || !tile
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                || !addresses.insert((&tile.source, tile.z, tile.x, tile.y))
            {
                return Err(ManifestError::Invalid(tile.file.clone()));
            }
            if bodies
                .insert(&tile.source, tile.body)
                .is_some_and(|body| body != tile.body)
            {
                return Err(ManifestError::Invalid(format!(
                    "{} spans multiple bodies",
                    tile.source
                )));
            }
        }
        Ok(result)
    }

    /// Default asset inventory for callers constructing a style without a host.
    pub fn bundled() -> Result<Self, ManifestError> {
        Self::parse(include_bytes!("../../../assets/basemap/manifest.json"))
    }

    /// Whether the publication includes physical imagery for a body.
    pub fn has_body(&self, body: BodyId) -> bool {
        self.tiles.iter().any(|tile| tile.body == body)
    }

    /// Layer IDs and their body, shared by style creation and body switching.
    pub fn layers(&self) -> Vec<(String, BodyId)> {
        self.groups()
            .into_iter()
            .filter_map(|((z, source), tiles)| {
                tiles
                    .first()
                    .map(|tile| (format!("basemap-{source}-{z}"), tile.body))
            })
            .collect()
    }

    pub(in crate::scene) fn apply(&self, style: &mut Value, body: BodyId) {
        for ((z, source), tiles) in self.groups() {
            let id = format!("basemap-{source}-{z}");
            let bounds = tiles.iter().map(|tile| tile.bounds()).fold(
                [180_f64, 90_f64, -180_f64, -90_f64],
                |[w, s, e, n], [west, south, east, north]| {
                    [w.min(west), s.min(south), e.max(east), n.max(north)]
                },
            );
            if let Some(sources) = style.get_mut("sources").and_then(Value::as_object_mut) {
                sources.insert(id.clone(), json!({"type": "raster", "tiles": [format!("atlas://{source}/{{z}}/{{x}}/{{y}}")],
                    "tileSize": 512, "minzoom": z, "maxzoom": z, "bounds": bounds}));
            }
            let visible = tiles.first().is_some_and(|tile| tile.body == body);
            if let Some(layers) = style.get_mut("layers").and_then(Value::as_array_mut) {
                let before = layers
                    .iter()
                    .position(|layer| layer.get("id").and_then(Value::as_str) == Some("graticule"))
                    .unwrap_or(layers.len());
                layers.insert(
                    before,
                    json!({"id": id, "type": "raster", "source": id, "minzoom": z,
                    "layout": {"visibility": if visible { "visible" } else { "none" }},
                    "paint": {"raster-fade-duration": 0}}),
                );
            }
        }
    }

    fn groups(&self) -> BTreeMap<(u8, &str), Vec<&Tile>> {
        let mut groups: BTreeMap<(u8, &str), Vec<&Tile>> = BTreeMap::new();
        for tile in &self.tiles {
            groups.entry((tile.z, &tile.source)).or_default().push(tile);
        }
        groups
    }
}

impl Tile {
    fn bounds(&self) -> [f64; 4] {
        let count = f64::from(1_u32 << self.z);
        let latitude = |y: f64| {
            (std::f64::consts::PI * (1. - 2. * y / count))
                .sinh()
                .atan()
                .to_degrees()
        };
        [
            f64::from(self.x) / count * 360. - 180.,
            latitude(f64::from(self.y) + 1.),
            (f64::from(self.x) + 1.) / count * 360. - 180.,
            latitude(f64::from(self.y)),
        ]
    }
}
