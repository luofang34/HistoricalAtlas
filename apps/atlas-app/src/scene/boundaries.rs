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
    include_derived: bool,
) -> Result<Vec<Value>, AppError> {
    if body != BodyId::Earth || layer == MapLayer::SelectedTime {
        return Ok(Vec::new());
    }
    let mut features: Vec<_> = atlas
        .data
        .entities
        .iter()
        .filter_map(|e| atlas.entry(e.id))
        .flat_map(|entry| &entry.boundaries)
        .map(|boundary| Ok(boundary.feature(&atlas.data, selected)?))
        .collect::<Result<_, AppError>>()?;
    if !include_derived {
        return Ok(features);
    }
    for boundary in atlas
        .data
        .entities
        .iter()
        .filter_map(|e| atlas.entry(e.id))
        .flat_map(|entry| &entry.derived_boundaries)
    {
        features.push(boundary.feature(&atlas.data, selected)?);
    }
    Ok(features)
}

/// Bounding-box midpoint for navigating to an entity with a registered boundary.
pub fn center(atlas: &Atlas, id: EntityId, include_derived: bool) -> Option<LonLat> {
    let [low, high] = extent(atlas, id, include_derived)?;
    Some(LonLat {
        longitude: (low.longitude + high.longitude) / 2.,
        latitude: (low.latitude + high.latitude) / 2.,
    })
}

/// Camera framing the complete registered boundary inside the map viewport.
pub fn frame(
    atlas: &Atlas,
    id: EntityId,
    viewport: [f64; 2],
    include_derived: bool,
) -> Option<crate::navigation::Camera> {
    let bounds = extent(atlas, id, include_derived)?;
    let mut camera = crate::navigation::Camera::for_body(BodyId::Earth);
    camera.fit_bounds(bounds, viewport);
    Some(camera)
}

fn extent(atlas: &Atlas, id: EntityId, include_derived: bool) -> Option<[LonLat; 2]> {
    let entry = atlas.entry(id)?;
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mapped = entry.boundaries.iter().flat_map(|boundary| {
        boundary
            .unit
            .polygons
            .iter()
            .flat_map(|p| &p.exterior)
            .filter_map(|index| boundary.vertices.get(*index))
    });
    let derived = entry
        .derived_boundaries
        .iter()
        .filter(|_| include_derived)
        .flat_map(|boundary| {
            boundary
                .region
                .polygons
                .iter()
                .flat_map(|p| &p.exterior)
                .filter_map(|index| boundary.region.vertices.get(*index))
        });
    for point in mapped.chain(derived) {
        bounds[0] = bounds[0].min(point.x);
        bounds[1] = bounds[1].min(point.y);
        bounds[2] = bounds[2].max(point.x);
        bounds[3] = bounds[3].max(point.y);
    }
    if !bounds.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some([
        LonLat {
            longitude: bounds[0],
            latitude: bounds[1],
        },
        LonLat {
            longitude: bounds[2],
            latitude: bounds[3],
        },
    ])
}

#[cfg(test)]
pub(crate) mod tests;
