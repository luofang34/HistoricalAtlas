use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use atlas_boundary::publication::{
    CatalogItem, CatalogShardSummary, ManifestError, PUBLICATION_SCHEMA_VERSION,
    PlateModelManifest, PublicationManifest, ShardSummary, VerifiedPart, decode_vec_parts,
};
use atlas_core::identity::EntityId;
use atlas_ledger::{model::Source, publication::Published};

use super::{Artifacts, ExportError, chunks, insert};
use crate::input::LedgerInput;

pub(super) fn publish_manifest(
    output: &mut Artifacts,
    input: &LedgerInput,
    current: &BTreeSet<EntityId>,
    shard_summaries: Vec<ShardSummary>,
    catalog_shards: Vec<CatalogShardSummary>,
) -> Result<(), ExportError> {
    let mut sources: Vec<_> = input.catalog.sources.iter().collect();
    sources.sort_by(|a, b| a.id.cmp(&b.id));
    let source_parts = chunks::insert_verified_array(output, "sources", &sources)?;
    let alias_parts = chunks::insert_verified_map(output, "entities/aliases", &input.aliases.0)?;
    let published = Published(input.published.0.union(current).copied().collect());
    let published_vec: Vec<_> = published.0.into_iter().collect();
    let published_parts =
        chunks::insert_verified_array(output, "entities/published", &published_vec)?;
    let shard_parts = chunks::insert_verified_array(output, "entities/shards", &shard_summaries)?;
    let mut plate_models = Vec::new();
    for (id, model) in &input.plate_models {
        let parts = chunks::insert_verified_parts(output, &format!("plates/{id}"), model)?;
        plate_models.push(PlateModelManifest {
            id: id.clone(),
            youngest_ma: model.youngest_ma,
            oldest_ma: model.oldest_ma,
            parts,
        });
    }
    let manifest = PublicationManifest {
        schema_version: PUBLICATION_SCHEMA_VERSION,
        sources: source_parts,
        aliases: alias_parts,
        published: published_parts,
        shard_inventories: shard_parts,
        catalog_shards,
        plate_models,
    };
    manifest.validate()?;
    insert(output, "manifest.json", &manifest)
}

/// Decode and validate `manifest.json` from an in-memory artifact bundle.
pub fn read_manifest(output: &Artifacts) -> Result<PublicationManifest, ExportError> {
    let bytes = output
        .get(Path::new("manifest.json"))
        .ok_or_else(|| ManifestError::Invalid("missing manifest.json".into()))?;
    let manifest: PublicationManifest = serde_json::from_slice(bytes)?;
    manifest.validate()?;
    Ok(manifest)
}

fn collect_parts(output: &Artifacts, parts: &[VerifiedPart]) -> Result<Vec<Vec<u8>>, ExportError> {
    parts
        .iter()
        .map(|part| {
            let bytes = output
                .get(Path::new(&part.path))
                .cloned()
                .ok_or_else(|| ManifestError::Checksum(part.path.clone()))?;
            Ok(bytes)
        })
        .collect()
}

/// Map every published entity identity to its `entities/index/...` relative shard path.
pub fn entity_shard_paths(output: &Artifacts) -> Result<BTreeMap<EntityId, String>, ExportError> {
    let manifest = read_manifest(output)?;
    let mut result = BTreeMap::new();
    for shard in &manifest.catalog_shards {
        let payloads = collect_parts(output, std::slice::from_ref(&shard.part))?;
        let items: Vec<CatalogItem> =
            decode_vec_parts(std::slice::from_ref(&shard.part), &payloads)?;
        for item in items {
            result.insert(item.id, format!("entities/index/{}.json", item.shard));
        }
    }
    Ok(result)
}

/// Resolve and decode one entity's raw index entry via `manifest.json` and its catalog shard.
pub fn lookup_entity(
    output: &Artifacts,
    id: EntityId,
) -> Result<Option<serde_json::Value>, ExportError> {
    let paths = entity_shard_paths(output)?;
    let Some(shard_path) = paths.get(&id) else {
        return Ok(None);
    };
    let bytes = output
        .get(Path::new(shard_path))
        .ok_or_else(|| ManifestError::Checksum(shard_path.clone()))?;
    let mut entries: BTreeMap<EntityId, serde_json::Value> = serde_json::from_slice(bytes)?;
    Ok(entries.remove(&id))
}

/// Verify and decode the complete source catalog from `manifest.json`.
pub fn load_sources(output: &Artifacts) -> Result<Vec<Source>, ExportError> {
    let manifest = read_manifest(output)?;
    let payloads = collect_parts(output, &manifest.sources)?;
    Ok(decode_vec_parts(&manifest.sources, &payloads)?)
}
