//! Source-map outlines with registration and boundary-construction evidence.

use crate::{catalog::Atlas, error::AppError, selection::SelectedTime};
use atlas_core::identity::EntityId;
use atlas_geo::{body::BodyId, coordinates::LonLat};
use serde_json::Value;

/// Select registered outlines on the subject's body and optional evidence-supported interval.
pub fn features(
    atlas: &Atlas,
    body: BodyId,
    time: Option<&SelectedTime>,
    selected: Option<EntityId>,
    include_derived: bool,
) -> Result<Vec<Value>, AppError> {
    let mut features: Vec<_> = atlas
        .data
        .entities
        .iter()
        .filter(|e| e.body == body && time.is_none_or(|time| !time.absent.contains(&e.id)))
        .filter_map(|e| atlas.entry(e.id))
        .flat_map(|entry| &entry.boundaries)
        .map(|boundary| Ok(boundary.feature(&atlas.data, selected)?))
        .collect::<Result<_, AppError>>()?;
    for entry in atlas
        .data
        .entities
        .iter()
        .filter(|e| e.body == body && time.is_none_or(|time| !time.absent.contains(&e.id)))
        .filter_map(|e| atlas.entry(e.id))
    {
        for line in &entry.lines {
            features.push(line.feature(&atlas.data, selected)?);
        }
    }
    if !include_derived {
        return select(features, time);
    }
    for boundary in atlas
        .data
        .entities
        .iter()
        .filter(|e| e.body == body && time.is_none_or(|time| !time.absent.contains(&e.id)))
        .filter_map(|e| atlas.entry(e.id))
        .flat_map(|entry| &entry.derived_boundaries)
    {
        features.push(boundary.feature(&atlas.data, selected)?);
    }
    select(features, time)
}

fn select(features: Vec<Value>, time: Option<&SelectedTime>) -> Result<Vec<Value>, AppError> {
    let Some(time) = time else {
        return Ok(features);
    };
    let mut selected = Vec::new();
    for mut feature in features {
        let span: Option<atlas_core::time::TemporalSpan> = serde_json::from_value(
            feature
                .pointer("/properties/validity")
                .cloned()
                .unwrap_or_default(),
        )?;
        if let Some(membership) = time.selection.select(span.as_ref(), time.at)? {
            if let Some(properties) = feature.get_mut("properties").and_then(Value::as_object_mut) {
                properties.insert("membership".into(), serde_json::to_value(membership)?);
            }
            selected.push(feature);
        }
    }
    Ok(selected)
}

/// Bounding-box midpoint for navigating to an entity with a registered boundary.
pub fn center(
    atlas: &Atlas,
    id: EntityId,
    include_derived: bool,
    time: Option<&SelectedTime>,
) -> Option<LonLat> {
    let [low, high] = extent(atlas, id, include_derived, time)?;
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
    time: Option<&SelectedTime>,
) -> Option<crate::navigation::Camera> {
    let bounds = extent(atlas, id, include_derived, time)?;
    let mut camera = crate::navigation::Camera::for_body(atlas.entry(id)?.entity.body);
    camera.fit_bounds(bounds, viewport);
    Some(camera)
}

fn extent(
    atlas: &Atlas,
    id: EntityId,
    include_derived: bool,
    time: Option<&SelectedTime>,
) -> Option<[LonLat; 2]> {
    let entry = atlas.entry(id)?;
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    if time.is_some_and(|time| time.absent.contains(&id)) {
        return None;
    }
    let visible = |span: Option<&atlas_core::time::TemporalSpan>| {
        time.is_none_or(|time| {
            time.selection
                .select(span, time.at)
                .ok()
                .flatten()
                .is_some()
        })
    };
    let mapped = entry
        .boundaries
        .iter()
        .filter(|b| visible(b.unit.validity.as_ref()))
        .flat_map(|boundary| {
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
        .filter(|b| include_derived && visible(b.validity.as_ref()))
        .flat_map(|boundary| {
            boundary
                .region
                .polygons
                .iter()
                .flat_map(|p| &p.exterior)
                .filter_map(|index| boundary.region.vertices.get(*index))
        });
    let lines = entry
        .lines
        .iter()
        .filter(|line| visible(line.line.validity.as_ref()))
        .flat_map(|line| {
            line.line
                .paths
                .iter()
                .flatten()
                .filter_map(|index| line.vertices.get(*index))
        });
    for point in mapped.chain(derived).chain(lines) {
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
