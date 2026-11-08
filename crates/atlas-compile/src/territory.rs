//! Static node territories with their complete source and relationship evidence.

use std::path::Path;

use atlas_boundary::nodal::{NodalBuilder, NodalError, NodalQuery};
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use serde::Serialize;
use thiserror::Error;

use crate::{
    export::{Artifacts, ExportError, compile, insert, write_blocking},
    input::{InputError, LedgerInput, load_blocking, read_json_blocking},
};

use crate::export::chunks::{ChunkKind, insert_chunks};

#[derive(Serialize)]
struct TerritoryManifest<'a> {
    geometry_kind: &'static str,
    query: &'a NodalQuery,
    entity_id: EntityId,
    body: BodyId,
    node_count: usize,
    link_count: usize,
    nodes: Vec<String>,
    links: Vec<String>,
    features: Vec<String>,
}

/// Compile a source-scoped or time-selected node territory and its evidence indexes.
pub fn compile_territory(
    input: &LedgerInput,
    query: NodalQuery,
) -> Result<Artifacts, TerritoryError> {
    let mut resolved = query.clone();
    resolved.entity_id = input
        .aliases
        .resolve(query.entity_id)
        .map_err(NodalError::from)?;
    let territory = NodalBuilder::new(&input.catalog, &input.data)?.build(resolved)?;
    let mut artifacts = compile(input)?;
    let nodes = insert_chunks(
        &mut artifacts,
        "territory/nodes",
        &territory.nodes,
        ChunkKind::Array,
    )?;
    let links = insert_chunks(
        &mut artifacts,
        "territory/links",
        &territory.links,
        ChunkKind::Array,
    )?;
    let features = insert_chunks(
        &mut artifacts,
        "territory/features",
        &territory.features(),
        ChunkKind::GeoJson,
    )?;
    let manifest = TerritoryManifest {
        geometry_kind: "nodal_territory",
        query: &query,
        entity_id: territory.query.entity_id,
        body: territory.body,
        node_count: territory.nodes.len(),
        link_count: territory.links.len(),
        nodes,
        links,
        features,
    };
    insert(&mut artifacts, "territory.json", &manifest)?;
    Ok(artifacts)
}

/// Load a query and publish a new directory after every artifact passes validation.
pub fn territory_blocking(
    root: &Path,
    request: &Path,
    output: &Path,
) -> Result<usize, TerritoryError> {
    let input = load_blocking(root)?;
    let query = read_json_blocking(request)?;
    let artifacts = compile_territory(&input, query)?;
    write_blocking(output, &artifacts)?;
    Ok(artifacts.len())
}

/// Input, territorial evidence, or publication failure.
#[derive(Debug, Error)]
pub enum TerritoryError {
    /// Ledger or request document failed to load.
    #[error("territory input: {0}")]
    Input(#[from] InputError),
    /// Node territory could not be resolved.
    #[error("territory network: {0}")]
    Nodal(#[from] NodalError),
    /// Static artifact validation or writing failed.
    #[error("territory export: {0}")]
    Export(#[from] ExportError),
}

#[cfg(test)]
mod tests;
