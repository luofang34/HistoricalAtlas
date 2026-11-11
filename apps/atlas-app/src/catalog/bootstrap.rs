use std::collections::{BTreeMap, BTreeSet};

use atlas_boundary::publication::{
    CatalogItem, GeometryReference, PublicationManifest, ShardSummary,
};
use atlas_core::identity::EntityId;
use atlas_geo::{body::BodyId, plates::RotationModel};
use atlas_ledger::{
    model::{Dataset, Entity, Source, SourceCatalog},
    plates::PlateModelDocument,
    publication::{Aliases, Published, verify_publication},
};
use serde::Deserialize;

use super::{Atlas, Entry};
use crate::error::AppError;

/// Compact startup documents enabling immediate search and on-demand shard streaming.
#[derive(Debug, Deserialize)]
pub struct Bootstrap {
    /// Verified root manifest declaring schema version and part checksums.
    pub manifest: PublicationManifest,
    /// Complete bibliographic catalog.
    pub sources: Vec<Source>,
    /// Redirects preserving external references to merged entity identities.
    #[serde(default)]
    pub aliases: Aliases,
    /// Complete published entity identity inventory.
    #[serde(default)]
    pub published: Published,
    /// Adaptive spatiotemporal and prefix shard summaries.
    pub shards: Vec<ShardSummary>,
    /// Search and hierarchy catalog items for every published entity.
    pub catalog: Vec<CatalogItem>,
    /// Cited plate-rotation documents keyed by model identifier.
    #[serde(default)]
    pub plate_models: BTreeMap<String, PlateModelDocument>,
}

/// Newly discovered state from attaching one streamed entity shard.
#[derive(Debug, Default)]
pub struct AttachedShard {
    /// Whether the shard introduces temporal constraints or plate assignments.
    pub refresh_time: bool,
    /// External geometry bundles referenced by newly attached entities.
    pub geometries: Vec<(EntityId, BodyId, GeometryReference)>,
}

pub(super) fn validate_entry_ownership(id: EntityId, entry: &Entry) -> Result<(), AppError> {
    if id != entry.entity.id {
        return Err(AppError::EntryIdentity {
            key: id,
            entity: entry.entity.id,
        });
    }
    if entry.records.iter().any(|record| record.entity_id != id)
        || entry
            .estimates
            .iter()
            .any(|estimate| estimate.entity_id != id)
        || entry.disputes.iter().any(|dispute| dispute.entity_id != id)
    {
        return Err(AppError::EvidenceIdentity(id));
    }
    if entry.records.iter().any(|record| !record.checked) {
        return Err(AppError::Unchecked(id));
    }
    Ok(())
}

pub(super) fn resolve_aliases(aliases: &Aliases) -> Result<BTreeMap<EntityId, EntityId>, AppError> {
    Ok(aliases
        .0
        .keys()
        .map(|id| aliases.resolve(*id).map(|target| (*id, target)))
        .collect::<Result<_, _>>()?)
}

pub(super) fn build_rotation_models(
    plate_models: &BTreeMap<String, PlateModelDocument>,
) -> Result<BTreeMap<String, RotationModel>, AppError> {
    let mut built = BTreeMap::new();
    for (id, document) in plate_models {
        if *id != document.id {
            return Err(AppError::PlateModel(id.clone()));
        }
        let model = RotationModel::from_sequences(document.sequences.clone())
            .map_err(|_| AppError::PlateModel(id.clone()))?;
        built.insert(id.clone(), model);
    }
    Ok(built)
}

fn topological_shard_order(
    shards: &BTreeMap<String, ShardSummary>,
) -> Result<Vec<String>, AppError> {
    let mut remaining: BTreeMap<&str, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut ready: BTreeSet<(bool, &str)> = BTreeSet::new();
    for shard in shards.values() {
        for dep in &shard.depends_on {
            let Some(target) = shards.get(dep) else {
                return Err(AppError::UnknownShard(dep.clone()));
            };
            dependents
                .entry(target.id.as_str())
                .or_default()
                .push(shard.id.as_str());
        }
        if shard.depends_on.is_empty() {
            ready.insert((!shard.overview, shard.id.as_str()));
        } else {
            remaining.insert(shard.id.as_str(), shard.depends_on.len());
        }
    }
    let mut ordered = Vec::with_capacity(shards.len());
    while let Some((_, id)) = ready.pop_first() {
        ordered.push(id.to_owned());
        if let Some(children) = dependents.get(id) {
            for &child in children {
                if let Some(count) = remaining.get_mut(child) {
                    *count = count.saturating_sub(1);
                    if *count == 0
                        && let Some(child_shard) = shards.get(child)
                    {
                        ready.insert((!child_shard.overview, child_shard.id.as_str()));
                    }
                }
            }
        }
    }
    if ordered.len() != shards.len() {
        return Err(AppError::UnknownShard("cyclic shard dependencies".into()));
    }
    Ok(ordered)
}

impl Atlas {
    /// Initialize an interactive atlas from verified startup metadata before leaf shards stream in.
    pub fn from_bootstrap(bootstrap: Bootstrap) -> Result<Self, AppError> {
        bootstrap.manifest.validate()?;
        let mut shard_summaries = BTreeMap::new();
        for shard in bootstrap.shards {
            shard.validate()?;
            if shard_summaries.insert(shard.id.clone(), shard).is_some() {
                return Err(AppError::UnknownShard("duplicate shard summary".into()));
            }
        }
        topological_shard_order(&shard_summaries)?;
        let mut data = Dataset::default();
        let mut catalog = BTreeMap::new();
        for item in bootstrap.catalog {
            if !shard_summaries.contains_key(&item.shard) {
                return Err(AppError::UnknownShard(item.shard));
            }
            let id = item.id;
            data.entities.push(Entity {
                id,
                key: item.key.clone(),
                body: item.body,
                kind: item.kind,
            });
            if catalog.insert(id, item).is_some() {
                return Err(AppError::DuplicateEntity(id));
            }
        }
        let sources = SourceCatalog {
            sources: bootstrap.sources,
        };
        atlas_ledger::validation::validate(&sources, &data)?;
        verify_publication(&data, &bootstrap.aliases, &bootstrap.published)?;
        let aliases = resolve_aliases(&bootstrap.aliases)?;
        let rotation_models = build_rotation_models(&bootstrap.plate_models)?;
        Ok(Self {
            sources,
            data,
            plate_models: bootstrap.plate_models,
            rotation_models,
            entries: BTreeMap::new(),
            aliases,
            catalog,
            shard_summaries,
            loaded_shards: BTreeSet::new(),
        })
    }

    /// Verify and attach one streamed entity shard, returning whether time selection needs refresh
    /// and any newly discovered external geometry references.
    pub fn attach_shard(
        &mut self,
        shard_id: &str,
        bytes: &[u8],
    ) -> Result<AttachedShard, AppError> {
        if self.loaded_shards.contains(shard_id) {
            return Ok(AttachedShard::default());
        }
        let summary = self
            .shard_summaries
            .get(shard_id)
            .ok_or_else(|| AppError::UnknownShard(shard_id.to_owned()))?;
        summary.part.verify(bytes)?;
        let needs_time = summary.temporal.has_temporal || !summary.temporal.plates.is_empty();
        let decoded: BTreeMap<EntityId, Entry> = serde_json::from_slice(bytes)?;
        let mut records = Vec::new();
        let mut estimates = Vec::new();
        let mut disputes = Vec::new();
        for (id, entry) in &decoded {
            validate_entry_ownership(*id, entry)?;
            if self.entries.contains_key(id) {
                return Err(AppError::DuplicateEntity(*id));
            }
            let Some(item) = self.catalog.get(id) else {
                return Err(AppError::EvidenceIdentity(*id));
            };
            if item.shard != shard_id
                || item.key != entry.entity.key
                || item.body != entry.entity.body
                || item.kind != entry.entity.kind
            {
                return Err(AppError::EvidenceIdentity(*id));
            }
            records.extend(entry.records.clone());
            estimates.extend(entry.estimates.clone());
            disputes.extend(entry.disputes.clone());
        }
        atlas_ledger::validation::validate_append(
            &self.sources,
            &self.data,
            &records,
            &estimates,
            &disputes,
        )?;
        let counts = (
            self.data.records.len(),
            self.data.estimates.len(),
            self.data.disputes.len(),
        );
        self.data.records.extend(records);
        self.data.estimates.extend(estimates);
        self.data.disputes.extend(disputes);
        for entry in decoded.values() {
            if let Err(error) = entry.validate_geometry(&self.sources, &self.data) {
                self.data.records.truncate(counts.0);
                self.data.estimates.truncate(counts.1);
                self.data.disputes.truncate(counts.2);
                return Err(error);
            }
        }
        let mut geometries = Vec::new();
        for (id, entry) in decoded {
            if let Some(reference) = &entry.geometry {
                geometries.push((id, entry.entity.body, reference.clone()));
            }
            self.entries.insert(id, entry);
        }
        self.loaded_shards.insert(shard_id.to_owned());
        Ok(AttachedShard {
            refresh_time: needs_time,
            geometries,
        })
    }

    /// All shard summaries in dependency-safe overview-first order.
    pub fn ordered_shards(&self) -> Result<Vec<&ShardSummary>, AppError> {
        topological_shard_order(&self.shard_summaries).map(|ids| {
            ids.into_iter()
                .filter_map(|id| self.shard_summaries.get(&id))
                .collect()
        })
    }

    /// Whether every cross-shard dependency of `shard` has already been attached.
    pub fn is_shard_ready(&self, shard: &ShardSummary) -> bool {
        shard
            .depends_on
            .iter()
            .all(|dep| self.loaded_shards.contains(dep))
    }

    /// Unloaded shard summaries matching `id` and its transitive cross-shard dependencies.
    pub fn unloaded_with_dependencies(&self, shard_id: &str, out: &mut BTreeSet<String>) {
        let mut stack = vec![shard_id];
        while let Some(current) = stack.pop() {
            if self.loaded_shards.contains(current) || !out.insert(current.to_owned()) {
                continue;
            }
            if let Some(summary) = self.shard_summaries.get(current) {
                for dep in &summary.depends_on {
                    stack.push(dep.as_str());
                }
            }
        }
    }

    /// Unloaded shard summaries on `body` in deterministic manifest order.
    pub fn unloaded_shards(&self, body: BodyId) -> impl Iterator<Item = &ShardSummary> {
        self.shard_summaries
            .values()
            .filter(move |s| s.body == body && !self.loaded_shards.contains(&s.id))
    }

    /// Whether any overview shard on `body` has not yet been attached.
    pub fn has_unloaded_overview_shards(&self, body: BodyId) -> bool {
        self.unloaded_shards(body).any(|shard| shard.overview)
    }

    /// Unloaded shard identifier owning `id`, if `id` is not yet loaded.
    pub fn unloaded_entity_shard(&self, id: EntityId) -> Option<&str> {
        let resolved = self.resolve(id)?;
        let shard = self.catalog.get(&resolved)?.shard.as_str();
        (!self.loaded_shards.contains(shard)).then_some(shard)
    }

    /// Envelope of all located shards on `body`, available immediately from bootstrap metadata.
    pub fn body_shard_bounds(&self, body: BodyId) -> Option<[f64; 4]> {
        self.shard_summaries
            .values()
            .filter(|shard| shard.body == body)
            .filter_map(|shard| shard.bounds)
            .reduce(|[w1, s1, e1, n1], [w2, s2, e2, n2]| {
                [w1.min(w2), s1.min(s2), e1.max(e2), n1.max(n2)]
            })
    }
}

#[cfg(test)]
mod tests;
