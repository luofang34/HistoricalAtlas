//! Versioned publication manifest, verified multi-part descriptors, and spatiotemporal shard summaries.

use std::collections::BTreeMap;

use atlas_core::{
    assertion::AdministrativeRank, identity::EntityId, taxonomy::EntityKind, time::EpochTime,
};
use atlas_geo::{body::BodyId, plates::PlateId};
use atlas_ledger::plates::PlateModelDocument;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

use super::fingerprint;

/// Supported wire schema version for compiled atlas publications.
pub const PUBLICATION_SCHEMA_VERSION: u32 = 1;

/// Exclusive byte ceiling for every static publication file.
pub const MAX_PUBLICATION_PART_BYTES: usize = 4_000_000;

/// Static publication file or byte-range part with strict size and SHA-256 metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedPart {
    /// Publication-relative POSIX path.
    pub path: String,
    /// Exact UTF-8 byte count, strictly in `1..4_000_000`.
    pub bytes: usize,
    /// Lowercase 64-character hexadecimal SHA-256 digest.
    pub sha256: String,
}

impl VerifiedPart {
    /// Build and validate a verified descriptor for `content` under `prefix`.
    pub fn from_bytes(
        path: impl Into<String>,
        prefix: &str,
        content: &[u8],
    ) -> Result<Self, ManifestError> {
        let part = Self {
            path: path.into(),
            bytes: content.len(),
            sha256: fingerprint(content),
        };
        part.validate_with_prefix(prefix)?;
        Ok(part)
    }

    /// Validate relative path structure, allowed prefix, byte bounds, and hex digest syntax.
    pub fn validate_with_prefix(&self, prefix: &str) -> Result<(), ManifestError> {
        if !self.path.starts_with(prefix)
            || self
                .path
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
            || !self
                .path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
            || self.bytes == 0
            || self.bytes >= MAX_PUBLICATION_PART_BYTES
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ManifestError::Invalid(format!(
                "invalid verified part descriptor {:?}",
                self.path
            )));
        }
        Ok(())
    }

    /// Verify exact byte length and SHA-256 digest against loaded bytes.
    pub fn verify(&self, content: &[u8]) -> Result<(), ManifestError> {
        if content.len() != self.bytes || fingerprint(content) != self.sha256 {
            return Err(ManifestError::Checksum(self.path.clone()));
        }
        Ok(())
    }
}

/// Plate model and moving-plate pair present inside a shard for rotated viewport bounds.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ShardPlateRef {
    /// Rotation model identifier.
    pub model_id: String,
    /// Moving plate identifier in that model.
    pub plate_id: PlateId,
}

/// Conservative temporal envelope for selective time-slice shard streaming.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardTemporalSummary {
    /// Whether any entity in the shard carries temporal assertions, origins, or dated geometry.
    #[serde(default)]
    pub has_temporal: bool,
    /// Whether any temporal constraint in the shard is unbounded toward the past.
    #[serde(default)]
    pub open_start: bool,
    /// Earliest finite TT tick across bounded intervals and origins in the shard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earliest: Option<EpochTime>,
    /// Latest finite TT tick across bounded intervals in the shard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<EpochTime>,
    /// Whether any temporal constraint or origin in the shard is unbounded toward the future.
    #[serde(default)]
    pub open_end: bool,
    /// Distinct plate assignments carried by entities in the shard.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plates: Vec<ShardPlateRef>,
}

impl ShardTemporalSummary {
    /// Whether any entity in this shard can appear in a time slice at `at`.
    pub fn matches_selected_time(
        &self,
        at: EpochTime,
        models: &BTreeMap<String, PlateModelDocument>,
    ) -> bool {
        if self.plates.iter().any(|plate| {
            models
                .get(&plate.model_id)
                .and_then(|model| model.age_at(at))
                .is_some()
        }) {
            return true;
        }
        if !self.has_temporal {
            return false;
        }
        let after_start = self.open_start || self.earliest.is_some_and(|start| at >= start);
        let before_end = self.open_end || self.latest.is_some_and(|end| at <= end);
        after_start && before_end
    }
}

/// Spatiotemporal and content-addressed metadata for one `entities/index/...` shard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShardSummary {
    /// Shard identifier matching `entities/index/{id}.json`.
    pub id: String,
    /// Verified artifact descriptor.
    pub part: VerifiedPart,
    /// Celestial body of all entities in this shard.
    pub body: BodyId,
    /// Whether this shard belongs to the overview tier loaded ahead of viewport details.
    pub overview: bool,
    /// Present-day `[west, south, east, north]` envelope in degrees, if any entity is located.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[f64; 4]>,
    /// Conservative temporal and plate-assignment summary.
    pub temporal: ShardTemporalSummary,
    /// Other shard identifiers whose checked records must be attached before this shard's estimates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Number of entities stored in this shard.
    pub entity_count: usize,
}

impl ShardSummary {
    /// Validate shard path consistency, geographic bounds, and non-empty entity count.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let expected = format!("entities/index/{}.json", self.id);
        if self.id.is_empty()
            || self.part.path != expected
            || self.entity_count == 0
            || self
                .depends_on
                .iter()
                .any(|dep| dep.is_empty() || dep == &self.id)
        {
            return Err(ManifestError::Invalid(format!(
                "invalid shard summary {:?}",
                self.id
            )));
        }
        self.part.validate_with_prefix("entities/index/")?;
        if self.bounds.is_some_and(|[w, s, e, n]| {
            ![w, s, e, n].iter().all(|v| v.is_finite())
                || w > e
                || s > n
                || w < -180.0
                || e > 180.0
                || s < -90.0
                || n > 90.0
        }) {
            return Err(ManifestError::Invalid(format!(
                "invalid shard bounds on {:?}",
                self.id
            )));
        }
        Ok(())
    }
}

/// Lightweight enduring-entity header enabling resolution, labels, hierarchy, and search before shard streaming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogItem {
    /// Permanent UUIDv5 entity identity.
    pub id: EntityId,
    /// Semantic key used to mint `id`.
    pub key: String,
    /// Celestial body containing the entity.
    pub body: BodyId,
    /// Domain-grouped geographical classification.
    pub kind: EntityKind,
    /// Shard identifier (`ShardSummary.id`) holding this entity's full evidence entry.
    pub shard: String,
    /// Primary display label (first non-empty source name, or `key`).
    pub label: String,
    /// Additional searchable strings (other names, pinyin, former names).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terms: Vec<String>,
    /// Administrative ranks stated in checked records or temporal hierarchy estimates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ranks: Vec<AdministrativeRank>,
    /// Source-reported parent entity identities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<EntityId>,
}

/// Verified prefix shard of `CatalogItem` headers (`entities/catalog/...`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogShardSummary {
    /// Lowercase hexadecimal UUID prefix (or `"all"` when unsplit).
    pub prefix: String,
    /// Verified artifact descriptor.
    pub part: VerifiedPart,
    /// Number of catalog items in this file.
    pub count: usize,
}

/// Multi-part descriptor for a published plate-rotation excerpt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlateModelManifest {
    /// Model identifier referenced by `EstimateValue::Plate`.
    pub id: String,
    /// Inclusive youngest covered age in Ma.
    pub youngest_ma: f64,
    /// Inclusive oldest covered age in Ma.
    pub oldest_ma: f64,
    /// Ordered byte parts under `plates/{id}/` assembled before JSON decoding.
    pub parts: Vec<VerifiedPart>,
}

/// Versioned root entry point (`manifest.json`) of a compiled atlas publication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicationManifest {
    /// Wire-format version (`PUBLICATION_SCHEMA_VERSION`).
    pub schema_version: u32,
    /// Chunked `Vec<Source>` parts under `sources/`.
    pub sources: Vec<VerifiedPart>,
    /// Chunked `BTreeMap<EntityId, EntityId>` redirect parts under `entities/aliases/`.
    pub aliases: Vec<VerifiedPart>,
    /// Chunked `Vec<EntityId>` published-identity parts under `entities/published/`.
    pub published: Vec<VerifiedPart>,
    /// Chunked `Vec<ShardSummary>` inventory parts under `entities/shards/`.
    pub shard_inventories: Vec<VerifiedPart>,
    /// Prefix-partitioned `Vec<CatalogItem>` headers under `entities/catalog/`.
    pub catalog_shards: Vec<CatalogShardSummary>,
    /// Multi-part `PlateModelDocument` descriptors under `plates/`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plate_models: Vec<PlateModelManifest>,
}

impl PublicationManifest {
    /// Validate schema version and every referenced part descriptor before fetching payloads.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != PUBLICATION_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedVersion {
                found: self.schema_version,
                supported: PUBLICATION_SCHEMA_VERSION,
            });
        }
        for (prefix, parts) in [
            ("sources/", &self.sources),
            ("entities/aliases/", &self.aliases),
            ("entities/published/", &self.published),
            ("entities/shards/", &self.shard_inventories),
        ] {
            if parts.is_empty() {
                return Err(ManifestError::Invalid(format!("missing {prefix} parts")));
            }
            for part in parts {
                part.validate_with_prefix(prefix)?;
            }
        }
        for shard in &self.catalog_shards {
            if shard.count == 0
                || shard.prefix.is_empty()
                || !(shard.prefix == "all"
                    || shard
                        .prefix
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
            {
                return Err(ManifestError::Invalid(format!(
                    "invalid catalog shard {:?}",
                    shard.prefix
                )));
            }
            shard.part.validate_with_prefix("entities/catalog/")?;
        }
        for model in &self.plate_models {
            let prefix = format!("plates/{}/", model.id);
            if model.id.is_empty()
                || model.parts.is_empty()
                || !model.youngest_ma.is_finite()
                || !model.oldest_ma.is_finite()
                || model.youngest_ma < 0.0
                || model.youngest_ma > model.oldest_ma
            {
                return Err(ManifestError::Invalid(format!(
                    "invalid plate model manifest {:?}",
                    model.id
                )));
            }
            for part in &model.parts {
                part.validate_with_prefix(&prefix)?;
            }
        }
        Ok(())
    }
}

/// Verify and concatenate ordered byte parts before JSON deserialization.
pub fn decode_concatenated<T: DeserializeOwned>(
    parts: &[VerifiedPart],
    payloads: &[Vec<u8>],
) -> Result<T, ManifestError> {
    if parts.is_empty() || parts.len() != payloads.len() {
        return Err(ManifestError::Invalid(
            "part count mismatch during concatenated decode".into(),
        ));
    }
    let mut bytes = Vec::new();
    for (part, content) in parts.iter().zip(payloads) {
        part.verify(content)?;
        bytes.extend_from_slice(content);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// Verify and decode standalone JSON array chunks into one vector.
pub fn decode_vec_parts<T: DeserializeOwned>(
    parts: &[VerifiedPart],
    payloads: &[Vec<u8>],
) -> Result<Vec<T>, ManifestError> {
    if parts.len() != payloads.len() {
        return Err(ManifestError::Invalid(
            "part count mismatch during vector decode".into(),
        ));
    }
    let mut combined = Vec::new();
    for (part, content) in parts.iter().zip(payloads) {
        part.verify(content)?;
        let items: Vec<T> = serde_json::from_slice(content)?;
        combined.extend(items);
    }
    Ok(combined)
}

/// Verify and decode standalone JSON object chunks into one map, rejecting duplicate keys.
pub fn decode_map_parts<K: Ord + DeserializeOwned, V: DeserializeOwned>(
    parts: &[VerifiedPart],
    payloads: &[Vec<u8>],
) -> Result<BTreeMap<K, V>, ManifestError> {
    if parts.len() != payloads.len() {
        return Err(ManifestError::Invalid(
            "part count mismatch during map decode".into(),
        ));
    }
    let mut combined = BTreeMap::new();
    for (part, content) in parts.iter().zip(payloads) {
        part.verify(content)?;
        let entries: BTreeMap<K, V> = serde_json::from_slice(content)?;
        for (key, value) in entries {
            if combined.insert(key, value).is_some() {
                return Err(ManifestError::DuplicateKey);
            }
        }
    }
    Ok(combined)
}

/// Invalid versioned publication manifest or corrupted part payload.
#[derive(Debug, Error)]
pub enum ManifestError {
    /// Publication uses an incompatible wire schema version.
    #[error("unsupported publication schema version {found}; expected {supported}")]
    UnsupportedVersion {
        /// Schema version declared by `manifest.json`.
        found: u32,
        /// Schema version supported by this reader.
        supported: u32,
    },
    /// Manifest paths, bounds, or byte counts violate publication invariants.
    #[error("invalid publication manifest: {0}")]
    Invalid(String),
    /// A part failed byte-length or SHA-256 verification.
    #[error("publication part failed length or checksum verification: {0}")]
    Checksum(String),
    /// Duplicate map key encountered across chunked map parts.
    #[error("duplicate key across chunked publication map parts")]
    DuplicateKey,
    /// JSON decoding failed after checksum verification.
    #[error("publication JSON: {0}")]
    Json(#[from] serde_json::Error),
}
