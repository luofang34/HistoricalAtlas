//! Online tile services a publication declares, each with its attribution and licence.

use atlas_geo::body::BodyId;
use serde::Deserialize;
use serde_json::{Value, json};

use super::ManifestError;

/// One declared tile service; the loader fetches nothing from undeclared hosts.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::scene) struct RemoteSource {
    pub id: String,
    #[serde(default = "super::earth")]
    pub body: BodyId,
    #[serde(rename = "type")]
    pub kind: RemoteKind,
    /// `terrarium` or `mapbox` RGB elevation encoding, for terrain only.
    #[serde(default)]
    pub encoding: Option<String>,
    /// Vertical exaggeration, for terrain only.
    #[serde(default)]
    pub exaggeration: Option<f32>,
    /// HTTPS URL template with `{z}`, `{x}` and `{y}`.
    pub tiles: String,
    pub tile_size: u32,
    #[serde(default)]
    pub minzoom: u8,
    pub maxzoom: u8,
    pub attribution: String,
    pub license: String,
}

/// Imagery drawn as a raster layer, or elevation that raises the surface and shades it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::scene) enum RemoteKind {
    Raster,
    Terrain,
}

impl RemoteSource {
    pub(super) fn validate(&self) -> Result<(), ManifestError> {
        let invalid = |reason: &str| ManifestError::Invalid(format!("{}: {reason}", self.id));
        if self.id.is_empty()
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err(invalid("identifier must be lowercase ASCII"));
        }
        if !self.tiles.starts_with("https://")
            || !["{z}", "{x}", "{y}"]
                .iter()
                .all(|part| self.tiles.contains(part))
        {
            return Err(invalid("tiles must be an HTTPS {z}/{x}/{y} template"));
        }
        if !matches!(self.tile_size, 256 | 512) || self.minzoom > self.maxzoom || self.maxzoom > 22
        {
            return Err(invalid("tile size or zoom range"));
        }
        if self.attribution.trim().is_empty() || self.license.trim().is_empty() {
            return Err(invalid("attribution and licence are required"));
        }
        let terrain_fields = match (self.encoding.as_deref(), self.exaggeration) {
            (Some("terrarium" | "mapbox"), Some(scale)) => scale.is_finite() && scale > 0.0,
            _ => false,
        };
        let none = self.encoding.is_none() && self.exaggeration.is_none();
        match self.kind {
            RemoteKind::Terrain if !terrain_fields => Err(invalid(
                "terrain needs an encoding and a positive exaggeration",
            )),
            RemoteKind::Raster if !none => Err(invalid("imagery takes no elevation fields")),
            _ => Ok(()),
        }
    }

    /// Whether a requested URL belongs to this service's template.
    pub(in crate::scene) fn serves(&self, url: &str) -> bool {
        self.tiles
            .split('{')
            .next()
            .is_some_and(|prefix| url.starts_with(prefix))
    }

    /// Layer identifiers shown only while this source's body is displayed.
    pub(super) fn layer_ids(&self) -> Vec<String> {
        match self.kind {
            RemoteKind::Raster => vec![self.id.clone()],
            RemoteKind::Terrain => vec![format!("{}-hillshade", self.id)],
        }
    }

    /// The style's `terrain` while `body` is displayed.
    pub(in crate::scene) fn terrain(&self, body: BodyId) -> Option<Value> {
        (self.kind == RemoteKind::Terrain && self.body == body)
            .then(|| json!({"source": self.id, "exaggeration": self.exaggeration}))
    }

    /// Add sources and layers below `before` in the style's layer list.
    pub(super) fn apply(&self, style: &mut Value, body: BodyId, before: usize) {
        let visibility = if self.body == body { "visible" } else { "none" };
        let source = |kind: &str, encoding: Option<&str>| {
            json!({"type": kind, "tiles": [self.tiles], "tileSize": self.tile_size,
                "minzoom": self.minzoom, "maxzoom": self.maxzoom,
                "attribution": self.attribution, "encoding": encoding})
        };
        let layer = match self.kind {
            RemoteKind::Raster => {
                insert_source(style, &self.id, source("raster", None));
                json!({"id": self.id, "type": "raster", "source": self.id,
                    "layout": {"visibility": visibility}})
            }
            RemoteKind::Terrain => {
                let encoding = self.encoding.as_deref();
                // A separate source for shading keeps its tiles independent of the
                // elevation tiles the surface mesh retains.
                let shading = format!("{}-hillshade", self.id);
                insert_source(style, &shading, source("raster-dem", encoding));
                insert_source(style, &self.id, source("raster-dem", encoding));
                if let (Some(terrain), Some(root)) = (self.terrain(body), style.as_object_mut()) {
                    root.insert("terrain".into(), terrain);
                }
                json!({"id": shading, "type": "hillshade", "source": shading,
                    "layout": {"visibility": visibility},
                    "paint": {"hillshade-shadow-color": "#473B24", "hillshade-exaggeration": 0.35}})
            }
        };
        if let Some(layers) = style.get_mut("layers").and_then(Value::as_array_mut) {
            layers.insert(before.min(layers.len()), layer);
        }
    }
}

fn insert_source(style: &mut Value, id: &str, source: Value) {
    if let Some(sources) = style.get_mut("sources").and_then(Value::as_object_mut) {
        sources.insert(id.to_owned(), source);
    }
}

#[cfg(test)]
mod tests;
