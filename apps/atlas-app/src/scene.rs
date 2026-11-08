//! Source-coordinate map features with independent temporal membership.

use crate::{catalog::Atlas, selection::SelectedTime};
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use serde_json::{Value, json};

pub mod renderer;

/// Visibility of coordinate observations in the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapLayer {
    /// All reviewed coordinates on the selected body.
    Observations,
    /// Source anchors belonging to entities with evidence at the selected time.
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
    let features: Vec<_> = atlas
        .data
        .records
        .iter()
        .filter_map(|record| {
            let entry = atlas.entry(record.entity_id)?;
            if entry.entity.body != body
                || (layer == MapLayer::SelectedTime
                    && !time.evidence.contains_key(&record.entity_id))
            {
                return None;
            }
            let coordinate = record.observation.coordinate.as_ref()?;
            Some(
                json!({"type": "Feature", "geometry": {"type": "Point", "coordinates":
                [coordinate.point.longitude, coordinate.point.latitude]}, "properties": {
                    "entity_id": record.entity_id, "record_id": record.id,
                    "name": record.observation.name, "selected": selected == Some(record.entity_id),
                    "geometry_role": "source_observation", "datum": coordinate.datum,
                    "resolution_arcminutes": coordinate.resolution_arcminutes,
                    "source_id": record.source_id, "page_or_sheet": record.page_or_sheet,
                    "scan_page": record.scan_page
                }}),
            )
        })
        .collect();
    json!({"type": "FeatureCollection", "features": features})
}

/// Style for source anchors, external events, and a latitude/longitude graticule.
pub fn style(points: Value) -> Result<maplibre::style::Style, serde_json::Error> {
    serde_json::from_value(json!({"version": 8, "center": [105.27, 27.3], "zoom": 10,
    "sources": {
        "observations": {"type": "geojson", "data": points},
        "grid": {"type": "geojson", "data": graticule()}
    }, "layers": [
        {"id": "background", "type": "background", "paint": {"background-color": "#152c35"}},
        {"id": "graticule", "type": "line", "source": "grid",
            "paint": {"line-color": "#31505c", "line-width": 1}},
        {"id": "observations", "type": "circle", "source": "observations",
            "filter": ["==", "geometry_role", "source_observation"],
            "paint": {"circle-radius": 5, "circle-color": "#e4bf80", "circle-stroke-width": 1,
                "circle-stroke-color": "#172e39"}},
        {"id": "external-events", "type": "circle", "source": "observations",
            "filter": ["==", "geometry_role", "external_event"],
            "paint": {"circle-radius": 10, "circle-color": "#43c5c3", "circle-opacity": 0.65,
                "circle-stroke-width": 2, "circle-stroke-color": "#bdf4e9"}},
        {"id": "selection", "type": "circle", "source": "observations",
            "filter": ["==", "selected", true],
            "paint": {"circle-radius": 8, "circle-color": "#ef8d55", "circle-stroke-width": 2,
                "circle-stroke-color": "#ffffff"}}
    ]}))
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
