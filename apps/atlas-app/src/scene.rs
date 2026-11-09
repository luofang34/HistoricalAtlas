//! Source-coordinate map features with independent temporal membership.

use crate::{catalog::Atlas, selection::SelectedTime};
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use serde_json::{Value, json};

pub mod basemap;
pub mod boundaries;
mod locations;
mod picking;
pub mod renderer;
pub mod sheet;
pub mod territories;

/// Visibility of coordinate observations in the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapLayer {
    /// All reviewed coordinates on the selected body.
    Observations,
    /// Derived locations with their registration evidence and positional allowance.
    MappedPlaces,
    /// Entities with evidence at the selected time, at their source anchors or,
    /// lacking one, at their registered map location.
    SelectedTime,
}

/// Build one point per source record, preserving coincident identities.
pub fn features(
    atlas: &Atlas,
    body: BodyId,
    time: &SelectedTime,
    layer: MapLayer,
    selected: Option<EntityId>,
) -> Value {
    if layer == MapLayer::MappedPlaces {
        return locations::features(atlas, body, selected);
    }
    let active = |id: EntityId| time.evidence.contains_key(&id) && !time.absent.contains(&id);
    let mut features: Vec<_> = atlas
        .data
        .records
        .iter()
        .filter_map(|record| {
            let entry = atlas.entry(record.entity_id)?;
            // A cited plate model places the coordinate at the selected age even
            // without dated evidence that the entity existed then.
            let paleo = (layer == MapLayer::SelectedTime)
                .then(|| time.paleo.get(&record.id))
                .flatten();
            if entry.entity.body != body
                || (layer == MapLayer::SelectedTime && !active(record.entity_id) && paleo.is_none())
            {
                return None;
            }
            let coordinate = record.observation.coordinate.as_ref()?;
            let mut properties =
                atlas_ledger::features::observation_properties(record, &entry.entity, coordinate);
            let point = paleo.map_or(coordinate.point, |position| position.reconstructed);
            if let Some(properties) = properties.as_object_mut() {
                properties.insert("selected".into(), json!(selected == Some(record.entity_id)));
                if let Some(position) = paleo {
                    properties.insert("reconstructed".into(), json!(true));
                    properties.insert("plate_model".into(), json!(position.model_id));
                    properties.insert("plate_id".into(), json!(position.plate_id));
                    properties.insert("age_ma".into(), json!(position.age_ma));
                    properties.insert("reference_frame".into(), json!(position.reference_frame));
                }
            }
            Some(
                json!({"type": "Feature", "geometry": {"type": "Point", "coordinates":
                [point.longitude, point.latitude]}, "properties": properties}),
            )
        })
        .collect();
    if layer == MapLayer::SelectedTime {
        // Entities placed only by a registered map or photograph still exist at
        // the selected time; their registered location stands in for an anchor.
        let anchored: std::collections::BTreeSet<EntityId> = atlas
            .data
            .records
            .iter()
            .filter(|record| record.observation.coordinate.is_some())
            .map(|record| record.entity_id)
            .collect();
        features.extend(locations::located(atlas, body, selected, &|id| {
            active(id) && !anchored.contains(&id)
        }));
    }
    json!({"type": "FeatureCollection", "features": features})
}

/// Style with body-specific physical geography and an explicit display projection.
pub fn style(
    points: Value,
    body: BodyId,
    projection: maplibre::projection::ProjectionType,
) -> Result<maplibre::style::Style, basemap::ManifestError> {
    style_with_basemap(points, body, projection, &basemap::Manifest::bundled()?)
}

/// Build a style using the host publication's physical imagery inventory.
pub fn style_with_basemap(
    points: Value,
    body: BodyId,
    projection: maplibre::projection::ProjectionType,
    manifest: &basemap::Manifest,
) -> Result<maplibre::style::Style, basemap::ManifestError> {
    let mut value = json!({"version": 8, "center": [0.0, 0.0], "zoom": 0.5,
    "projection": {"type": projection},
    "sources": {
        "observations": {"type": "geojson", "data": points},
        "selection": {"type": "geojson", "data": {"type": "FeatureCollection", "features": []}},
        "grid": {"type": "geojson", "data": graticule()}
    }, "layers": [
        {"id": "background", "type": "background", "paint": {"background-color": "#152c35"}},
        {"id": "graticule", "type": "line", "source": "grid",
            "paint": {"line-color": "#31505c", "line-width": 0.6, "line-opacity": 0.3}},
        {"id": "boundaries", "type": "fill", "source": "observations",
            "filter": ["==", "geometry_role", "registered_boundary"],
            "paint": {"fill-color": "#367985", "fill-opacity": 0.15}},
        {"id": "derived-boundaries", "type": "fill", "source": "observations",
            "filter": ["==", "geometry_role", "derived_boundary"],
            "paint": {"fill-color": "#9d742b", "fill-opacity": 0.1}},
        {"id": "derived-lines", "type": "line", "source": "observations",
            "filter": ["==", "geometry_role", "derived_boundary"],
            "paint": {"line-color": "#91621c", "line-width": 1.5, "line-dasharray": [3, 2]}},
        {"id": "derived-selection", "type": "line", "source": "selection",
            "filter": ["all", ["==", "geometry_role", "derived_boundary"], ["==", "selected", true]],
            "paint": {"line-color": "#e25c31", "line-width": 3, "line-dasharray": [3, 2]}},
        {"id": "boundary-lines", "type": "line", "source": "observations",
            "filter": ["all", ["==", "geometry_role", "registered_boundary"], ["!=", "membership", "possible"]],
            "paint": {"line-color": "#23616e", "line-width": 2}},
        {"id": "possible-boundary-lines", "type": "line", "source": "observations",
            "filter": ["all", ["==", "geometry_role", "registered_boundary"], ["==", "membership", "possible"]],
            "paint": {"line-color": "#23616e", "line-width": 2, "line-dasharray": [3, 2]}},
        {"id": "registered-lines", "type": "line", "source": "observations",
            "filter": ["==", "geometry_role", "registered_line"],
            "paint": {"line-color": "#5b9cac", "line-width": 2}},
        {"id": "line-selection", "type": "line", "source": "selection",
            "filter": ["==", "geometry_role", "registered_line"],
            "paint": {"line-color": "#f5bf65", "line-width": 4}},
        {"id": "boundary-selection", "type": "line", "source": "selection",
            "filter": ["all", ["==", "geometry_role", "registered_boundary"], ["==", "selected", true]],
            "paint": {"line-color": "#ef8d55", "line-width": 3}},
        {"id": "territory-links", "type": "line", "source": "selection",
            "filter": ["==", "geometry_role", "territory_link"],
            "paint": {"line-color": "#91b8cf", "line-width": 2, "line-dasharray": [3, 2]}},
        {"id": "territory-nodes", "type": "circle", "source": "selection",
            "filter": ["==", "geometry_role", "territory_node"],
            "paint": {"circle-radius": 8, "circle-color": "#31505c", "circle-stroke-width": 2,
                "circle-stroke-color": "#91b8cf"}},
        {"id": "observations", "type": "circle", "source": "observations",
            "filter": ["==", "geometry_role", "source_observation"],
            "paint": {"circle-radius": 4, "circle-color": "#8d3e32", "circle-stroke-width": 1,
                "circle-stroke-color": "#fff5dc"}},
        {"id": "mapped-places", "type": "circle", "source": "observations",
            "filter": ["==", "geometry_role", "location_estimate"],
            "paint": {"circle-radius": 5, "circle-color": "#176b78", "circle-stroke-width": 1.5,
                "circle-stroke-color": "#fff5dc"}},
        {"id": "external-events", "type": "circle", "source": "observations",
            "filter": ["==", "geometry_role", "external_event"],
            "paint": {"circle-radius": 10, "circle-color": "#43c5c3", "circle-opacity": 0.65,
                "circle-stroke-width": 2, "circle-stroke-color": "#bdf4e9"}},
        {"id": "selection", "type": "circle", "source": "selection",
            "filter": ["all", ["==", "$type", "Point"], ["==", "selected", true]],
            "paint": {"circle-radius": 8, "circle-color": "#ef8d55", "circle-stroke-width": 2,
                "circle-stroke-color": "#ffffff"}}
    ]});
    manifest.apply(&mut value, body);
    Ok(serde_json::from_value(value)?)
}

fn graticule() -> Value {
    let meridians = (-180..180).step_by(10).map(|lon| json!({"type": "Feature",
        "geometry": {"type": "LineString", "coordinates": [[lon, -85], [lon, 85]]}, "properties": {}}));
    let parallels = (-80..=80).step_by(10).map(|lat| json!({"type": "Feature",
        "geometry": {"type": "LineString", "coordinates": [[-180, lat], [0, lat], [180, lat]]}, "properties": {}}));
    json!({"type": "FeatureCollection", "features": meridians.chain(parallels).collect::<Vec<_>>()})
}

#[cfg(test)]
mod tests;
