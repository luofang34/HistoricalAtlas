//! Source-map outlines with registration and boundary-construction evidence.

use super::MapLayer;
use crate::{catalog::Atlas, error::AppError};
use atlas_core::identity::EntityId;
use atlas_geo::{body::BodyId, coordinates::LonLat};
use serde_json::Value;

/// Select registered outlines for source browsing on Earth.
pub fn features(
    atlas: &Atlas,
    body: BodyId,
    layer: MapLayer,
    selected: Option<EntityId>,
) -> Result<Vec<Value>, AppError> {
    if body != BodyId::Earth || layer == MapLayer::SelectedTime {
        return Ok(Vec::new());
    }
    atlas
        .data
        .entities
        .iter()
        .filter_map(|e| atlas.entry(e.id))
        .flat_map(|entry| &entry.boundaries)
        .map(|boundary| Ok(boundary.feature(&atlas.data, selected)?))
        .collect()
}

/// Bounding-box midpoint for navigating to an entity with a registered boundary.
pub fn center(atlas: &Atlas, id: EntityId) -> Option<LonLat> {
    let boundary = atlas.entry(id)?.boundaries.first()?;
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for point in boundary
        .unit
        .polygons
        .iter()
        .flat_map(|p| &p.exterior)
        .filter_map(|index| boundary.vertices.get(*index))
    {
        bounds[0] = bounds[0].min(point.x);
        bounds[1] = bounds[1].min(point.y);
        bounds[2] = bounds[2].max(point.x);
        bounds[3] = bounds[3].max(point.y);
    }
    Some(LonLat {
        longitude: (bounds[0] + bounds[2]) / 2.,
        latitude: (bounds[1] + bounds[3]) / 2.,
    })
}

#[cfg(test)]
pub(crate) mod tests;
