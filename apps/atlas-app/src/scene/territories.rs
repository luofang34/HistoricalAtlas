//! Source-edition or dated hierarchy networks for the selected place.

use std::collections::BTreeSet;

use atlas_boundary::nodal::{NodalBuilder, NodalError, NodalQuery, NodalScope};
use atlas_core::identity::EntityId;
use serde_json::Value;

use crate::{catalog::Atlas, scene::MapLayer, selection::SelectedTime};

/// Resolve each source edition independently, or use only time-selected hierarchy estimates.
pub fn features(
    atlas: &Atlas,
    id: EntityId,
    time: &SelectedTime,
    layer: MapLayer,
) -> Result<Vec<Value>, NodalError> {
    let scopes: Vec<_> = match layer {
        MapLayer::Observations | MapLayer::MappedPlaces => atlas
            .entry(id)
            .into_iter()
            .flat_map(|entry| &entry.records)
            .map(|record| record.source_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|source_id| NodalScope::Source { source_id })
            .collect(),
        MapLayer::SelectedTime => vec![NodalScope::Time {
            at: time.at,
            selection: time.selection,
        }],
    };
    let builder = NodalBuilder::new(&atlas.sources, &atlas.data)?;
    let mut features = Vec::new();
    for scope in scopes {
        let territory = builder.build(NodalQuery {
            entity_id: id,
            scope,
        })?;
        features.extend(territory.features());
    }
    Ok(features)
}

#[cfg(test)]
mod tests;
