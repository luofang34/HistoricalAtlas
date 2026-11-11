//! Reassemble published evidence shards and search enduring entity identities.

use std::collections::{BTreeMap, BTreeSet};

use atlas_boundary::publication::{CatalogItem, ShardSummary};
use atlas_core::{assertion::AdministrativeRank, identity::EntityId};
use atlas_geo::{body::BodyId, plates::RotationModel};
use atlas_ledger::model::{Dataset, Dispute, Entity, Estimate, Record, Source, SourceCatalog};
use atlas_ledger::publication::{Aliases, Published, verify_publication};
use serde::Deserialize;

use crate::error::AppError;

mod bootstrap;
mod geometry;

pub use bootstrap::{AttachedShard, Bootstrap};

/// A single entity's published evidence.
#[derive(Debug, Deserialize)]
pub struct Entry {
    /// External geometry parts loaded independently of the evidence index.
    #[serde(default)]
    pub geometry: Option<atlas_boundary::publication::GeometryReference>,
    /// Registered source linework, including rivers and infrastructure.
    #[serde(default)]
    pub lines: Vec<atlas_boundary::linear::GeographicLine>,
    /// Terrain-cost reconstructions with their source membership evidence.
    #[serde(default)]
    pub derived_boundaries: Vec<atlas_boundary::terrain::DerivedBoundary>,
    /// Registered source-map outlines in OGC:CRS84.
    #[serde(default)]
    pub boundaries: Vec<atlas_boundary::geographic::GeographicBoundary>,
    /// Reviewed vector drawings in the source map's pixel frame.
    #[serde(default)]
    pub drawings: Vec<atlas_boundary::sheet::SheetTrace>,
    /// Permanent geographical identity.
    pub entity: Entity,
    /// Reviewed source observations.
    pub records: Vec<Record>,
    /// Derived interpretations.
    pub estimates: Vec<Estimate>,
    /// Explicit source disagreements.
    pub disputes: Vec<Dispute>,
}

/// The publication documents assembled by an in-memory loader.
#[derive(Debug, Deserialize)]
pub struct Publication {
    /// Bibliography accompanying the entity indexes.
    pub sources: Vec<Source>,
    /// UUID-keyed entity evidence.
    pub entries: BTreeMap<EntityId, Entry>,
    /// Redirects preserving external references to merged entity identities.
    #[serde(default)]
    pub aliases: Aliases,
    /// Cited plate-rotation excerpts keyed by model identifier.
    #[serde(default)]
    pub plate_models: BTreeMap<String, atlas_ledger::plates::PlateModelDocument>,
}

/// Validated, searchable evidence shared by the native and browser applications.
pub struct Atlas {
    /// Bibliographic source catalog.
    pub sources: SourceCatalog,
    /// Evidence reconstructed from the published shards.
    pub data: Dataset,
    /// Plate models that place assigned coordinates in deep time.
    pub plate_models: BTreeMap<String, atlas_ledger::plates::PlateModelDocument>,
    rotation_models: BTreeMap<String, RotationModel>,
    entries: BTreeMap<EntityId, Entry>,
    aliases: BTreeMap<EntityId, EntityId>,
    catalog: BTreeMap<EntityId, CatalogItem>,
    shard_summaries: BTreeMap<String, ShardSummary>,
    loaded_shards: BTreeSet<String>,
}

impl Atlas {
    /// Validate publication identity and evidence before making it visible.
    pub fn new(publication: Publication) -> Result<Self, AppError> {
        let sources = SourceCatalog {
            sources: publication.sources,
        };
        let mut data = Dataset::default();
        for (id, entry) in &publication.entries {
            bootstrap::validate_entry_ownership(*id, entry)?;
            data.entities.push(entry.entity.clone());
            data.records.extend(entry.records.clone());
            data.estimates.extend(entry.estimates.clone());
            data.disputes.extend(entry.disputes.clone());
        }
        atlas_ledger::validation::validate(&sources, &data)?;
        for entry in publication.entries.values() {
            entry.validate_geometry(&sources, &data)?;
        }
        verify_publication(&data, &publication.aliases, &Published::default())?;
        let aliases = bootstrap::resolve_aliases(&publication.aliases)?;
        let rotation_models = bootstrap::build_rotation_models(&publication.plate_models)?;
        Ok(Self {
            sources,
            data,
            plate_models: publication.plate_models,
            rotation_models,
            entries: publication.entries,
            aliases,
            catalog: BTreeMap::new(),
            shard_summaries: BTreeMap::new(),
            loaded_shards: BTreeSet::new(),
        })
    }

    /// Pre-validated plate kinematic models cached at startup.
    pub fn rotation_models(&self) -> &BTreeMap<String, RotationModel> {
        &self.rotation_models
    }

    /// Find an entity and its evidence by permanent identity.
    pub fn entry(&self, id: EntityId) -> Option<&Entry> {
        self.entries.get(&self.resolve(id)?)
    }

    /// Planetary body of a known entity, available even before leaf shards stream in.
    pub fn entity_body(&self, id: EntityId) -> Option<BodyId> {
        let id = self.resolve(id)?;
        self.entries
            .get(&id)
            .map(|entry| entry.entity.body)
            .or_else(|| self.catalog.get(&id).map(|item| item.body))
    }

    /// Canonical semantic key of a known entity, available before leaf shards stream in.
    pub fn entity_key(&self, id: EntityId) -> Option<&str> {
        let id = self.resolve(id)?;
        self.entries
            .get(&id)
            .map(|entry| entry.entity.key.as_str())
            .or_else(|| self.catalog.get(&id).map(|item| item.key.as_str()))
    }

    /// Ontological category of a known entity, available before leaf shards stream in.
    pub fn entity_kind(&self, id: EntityId) -> Option<atlas_core::taxonomy::EntityKind> {
        let id = self.resolve(id)?;
        self.entries
            .get(&id)
            .map(|entry| entry.entity.kind)
            .or_else(|| self.catalog.get(&id).map(|item| item.kind))
    }

    /// Administrative ranks stated for `id` in checked observations or hierarchy estimates.
    pub fn entity_ranks(&self, id: EntityId) -> Vec<AdministrativeRank> {
        let Some(id) = self.resolve(id) else {
            return Vec::new();
        };
        if let Some(item) = self.catalog.get(&id) {
            return item.ranks.clone();
        }
        let Some(entry) = self.entries.get(&id) else {
            return Vec::new();
        };
        let mut ranks = Vec::new();
        for rank in entry
            .records
            .iter()
            .filter_map(|record| record.observation.rank)
            .chain(
                entry
                    .estimates
                    .iter()
                    .filter_map(|estimate| match &estimate.value {
                        atlas_ledger::model::EstimateValue::Temporal { assertion } => {
                            match &assertion.value {
                                atlas_core::assertion::TemporalValue::Hierarchy(h) => Some(h.rank),
                                _ => None,
                            }
                        }
                        _ => None,
                    }),
            )
        {
            if !ranks.contains(&rank) {
                ranks.push(rank);
            }
        }
        ranks
    }

    /// Every enduring entity identity published on `body`.
    pub fn body_entities(&self, body: BodyId) -> Vec<EntityId> {
        if !self.catalog.is_empty() {
            return self
                .catalog
                .iter()
                .filter(|(_, item)| item.body == body)
                .map(|(id, _)| *id)
                .collect();
        }
        self.entries
            .iter()
            .filter(|(_, entry)| entry.entity.body == body)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Every `(parent, child)` containment edge among entities on `body`.
    pub fn parent_links(&self, body: BodyId) -> Vec<(EntityId, EntityId)> {
        if !self.catalog.is_empty() {
            return self
                .catalog
                .iter()
                .filter(|(_, item)| item.body == body)
                .flat_map(|(child, item)| {
                    item.parents
                        .iter()
                        .filter_map(|parent| self.resolve(*parent))
                        .filter(|parent| self.entity_body(*parent) == Some(body))
                        .map(move |parent| (parent, *child))
                })
                .collect();
        }
        self.entries
            .iter()
            .filter(|(_, entry)| entry.entity.body == body)
            .flat_map(|(child, entry)| {
                self.entry_parents(entry)
                    .into_iter()
                    .filter(|parent| self.entity_body(*parent) == Some(body))
                    .map(move |parent| (parent, *child))
            })
            .collect()
    }

    /// Whether reviewed observations or dated hierarchy assign `rank` to `id`.
    pub fn has_rank(&self, id: EntityId, rank: AdministrativeRank) -> bool {
        let Some(id) = self.resolve(id) else {
            return false;
        };
        if let Some(item) = self.catalog.get(&id) {
            return item.ranks.contains(&rank);
        }
        self.entries.get(&id).is_some_and(|entry| {
            entry
                .records
                .iter()
                .any(|record| record.observation.rank == Some(rank))
        })
    }

    /// Source drawings containing a selected boundary or place.
    pub fn drawings(&self, id: EntityId) -> Vec<&atlas_boundary::sheet::SheetTrace> {
        let records: BTreeSet<_> = self
            .entry(id)
            .into_iter()
            .flat_map(|e| &e.records)
            .map(|r| r.id)
            .collect();
        self.entries
            .values()
            .flat_map(|e| &e.drawings)
            .filter(|d| {
                d.units.iter().any(|u| records.contains(&u.record_id))
                    || d.lines.iter().any(|line| records.contains(&line.record_id))
                    || d.places.iter().any(|p| records.contains(&p.record_id))
            })
            .collect()
    }

    /// Resolve a permanent identity or published redirect to a current entity.
    pub fn resolve(&self, id: EntityId) -> Option<EntityId> {
        (self.entries.contains_key(&id) || self.catalog.contains_key(&id))
            .then_some(id)
            .or_else(|| self.aliases.get(&id).copied())
    }

    /// Display the first published name for source browsing.
    pub fn label(&self, id: EntityId) -> String {
        let Some(resolved) = self.resolve(id) else {
            return id.0.to_string();
        };
        if let Some(entry) = self.entries.get(&resolved) {
            return entry
                .records
                .iter()
                .find(|record| !record.observation.name.is_empty())
                .map_or_else(
                    || entry.entity.key.clone(),
                    |record| record.observation.name.clone(),
                );
        }
        self.catalog
            .get(&resolved)
            .map_or_else(|| resolved.0.to_string(), |item| item.label.clone())
    }

    /// Search standard, pinyin, former names, semantic keys, and UUIDs on one body.
    pub fn search(&self, query: &str, body: BodyId) -> Vec<EntityId> {
        let query = query.trim().to_lowercase();
        let alias = self
            .aliases
            .iter()
            .find_map(|(id, target)| (id.0.to_string() == query).then_some(*target));
        if !self.catalog.is_empty() {
            return self
                .catalog
                .iter()
                .filter(|(id, item)| {
                    item.body == body
                        && (query.is_empty()
                            || alias == Some(**id)
                            || id.0.to_string().contains(&query)
                            || item.key.to_lowercase().contains(&query)
                            || item.label.to_lowercase().contains(&query)
                            || item
                                .terms
                                .iter()
                                .any(|term| term.to_lowercase().contains(&query)))
                })
                .map(|(id, _)| *id)
                .collect();
        }
        self.entries
            .iter()
            .filter(|(id, entry)| {
                entry.entity.body == body
                    && (query.is_empty()
                        || alias == Some(**id)
                        || id.0.to_string().contains(&query)
                        || entry.entity.key.to_lowercase().contains(&query)
                        || entry.records.iter().any(|record| {
                            record.observation.name.to_lowercase().contains(&query)
                                || record
                                    .observation
                                    .pinyin
                                    .as_ref()
                                    .is_some_and(|value| value.to_lowercase().contains(&query))
                                || record
                                    .observation
                                    .former_names
                                    .iter()
                                    .any(|value| value.name.to_lowercase().contains(&query))
                        }))
            })
            .map(|(id, _)| *id)
            .collect()
    }

    fn entry_parents(&self, entry: &Entry) -> BTreeSet<EntityId> {
        entry
            .records
            .iter()
            .filter_map(|record| record.observation.parent_id)
            .filter_map(|parent| self.resolve(parent))
            .collect()
    }

    /// Source-reported parents, retaining different undated claims.
    pub fn parents(&self, id: EntityId) -> BTreeSet<EntityId> {
        let Some(resolved) = self.resolve(id) else {
            return BTreeSet::new();
        };
        if let Some(item) = self.catalog.get(&resolved) {
            return item
                .parents
                .iter()
                .filter_map(|parent| self.resolve(*parent))
                .collect();
        }
        self.entries
            .get(&resolved)
            .map_or_else(BTreeSet::new, |entry| self.entry_parents(entry))
    }

    /// Entities that a reviewed source places below this parent.
    pub fn children(&self, parent: EntityId) -> Vec<EntityId> {
        let Some(parent) = self.resolve(parent) else {
            return Vec::new();
        };
        if !self.catalog.is_empty() {
            return self
                .catalog
                .iter()
                .filter(|(_, item)| {
                    item.parents
                        .iter()
                        .any(|candidate| self.resolve(*candidate) == Some(parent))
                })
                .map(|(id, _)| *id)
                .collect();
        }
        self.entries
            .iter()
            .filter(|(_, entry)| self.entry_parents(entry).contains(&parent))
            .map(|(id, _)| *id)
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests;
