//! Reassemble published evidence shards and search enduring entity identities.

use std::collections::{BTreeMap, BTreeSet};

use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use atlas_ledger::model::{Dataset, Dispute, Entity, Estimate, Record, Source, SourceCatalog};
use atlas_ledger::publication::{Aliases, Published, verify_publication};
use serde::Deserialize;

use crate::error::AppError;
mod geometry;

/// A single entity's published evidence.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// The publication documents assembled by a filesystem or browser loader.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    /// Bibliography accompanying the entity indexes.
    pub sources: Vec<Source>,
    /// UUID-keyed entity evidence.
    pub entries: BTreeMap<EntityId, Entry>,
    /// Redirects preserving external references to merged entity identities.
    #[serde(default)]
    pub aliases: Aliases,
}

/// Validated, searchable evidence shared by the native and browser applications.
pub struct Atlas {
    /// Bibliographic source catalog.
    pub sources: SourceCatalog,
    /// Evidence reconstructed from the published shards.
    pub data: Dataset,
    entries: BTreeMap<EntityId, Entry>,
    aliases: BTreeMap<EntityId, EntityId>,
}

impl Atlas {
    /// Validate publication identity and evidence before making it visible.
    pub fn new(publication: Publication) -> Result<Self, AppError> {
        let sources = SourceCatalog {
            sources: publication.sources,
        };
        let mut data = Dataset::default();
        for (id, entry) in &publication.entries {
            if *id != entry.entity.id {
                return Err(AppError::EntryIdentity {
                    key: *id,
                    entity: entry.entity.id,
                });
            }
            if entry.records.iter().any(|record| record.entity_id != *id)
                || entry
                    .estimates
                    .iter()
                    .any(|estimate| estimate.entity_id != *id)
                || entry
                    .disputes
                    .iter()
                    .any(|dispute| dispute.entity_id != *id)
            {
                return Err(AppError::EvidenceIdentity(*id));
            }
            if entry.records.iter().any(|record| !record.checked) {
                return Err(AppError::Unchecked(*id));
            }
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
        let aliases = publication
            .aliases
            .0
            .keys()
            .map(|id| publication.aliases.resolve(*id).map(|target| (*id, target)))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            sources,
            data,
            entries: publication.entries,
            aliases,
        })
    }

    /// Find an entity and its evidence by permanent identity.
    pub fn entry(&self, id: EntityId) -> Option<&Entry> {
        self.entries.get(&self.resolve(id)?)
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
        self.entries
            .contains_key(&id)
            .then_some(id)
            .or_else(|| self.aliases.get(&id).copied())
    }

    /// Display the first published name for source browsing.
    pub fn label(&self, id: EntityId) -> String {
        self.entry(id).map_or_else(
            || id.0.to_string(),
            |entry| {
                entry
                    .records
                    .iter()
                    .find(|record| !record.observation.name.is_empty())
                    .map_or_else(
                        || entry.entity.key.clone(),
                        |record| record.observation.name.clone(),
                    )
            },
        )
    }

    /// Search standard, pinyin, former names, semantic keys, and UUIDs on one body.
    pub fn search(&self, query: &str, body: BodyId) -> Vec<EntityId> {
        let query = query.trim().to_lowercase();
        let alias = self
            .aliases
            .iter()
            .find_map(|(id, target)| (id.0.to_string() == query).then_some(*target));
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

    /// Source-reported parents, retaining different undated claims.
    pub fn parents(&self, id: EntityId) -> BTreeSet<EntityId> {
        self.entry(id)
            .into_iter()
            .flat_map(|entry| &entry.records)
            .filter_map(|record| record.observation.parent_id)
            .collect()
    }

    /// Entities that a reviewed source places below this parent.
    pub fn children(&self, parent: EntityId) -> Vec<EntityId> {
        self.entries
            .iter()
            .filter(|(_, entry)| {
                entry
                    .records
                    .iter()
                    .any(|record| record.observation.parent_id == Some(parent))
            })
            .map(|(id, _)| *id)
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests;
