//! Shared feature properties for interactive maps and static vector tiles.

use crate::model::{Entity, Estimate, EstimateValue, Record};
use atlas_geo::coordinates::CoordinateObservation;
use serde_json::{Value, json};

/// Cited coordinate properties with source notation and reporting precision intact.
pub fn observation_properties(
    record: &Record,
    entity: &Entity,
    coordinate: &CoordinateObservation,
) -> Value {
    json!({
        "entity_id": entity.id, "record_id": record.id, "entity_kind": entity.kind,
        "entity_domain": entity.kind.domain(), "entity_subkind": entity.kind.subkind(),
        "name": if record.observation.name.is_empty() { &entity.key } else { &record.observation.name },
        "pinyin": record.observation.pinyin, "population": record.observation.population,
        "households": record.observation.households, "rank": record.observation.rank,
        "parent_id": record.observation.parent_id, "seat_id": record.observation.seat_id,
        "source_id": record.source_id, "page_or_sheet": record.page_or_sheet,
        "scan_page": record.scan_page, "locator": record.locator, "source_date": record.source_date,
        "longitude": coordinate.point.longitude, "latitude": coordinate.point.latitude,
        "longitude_text": coordinate.longitude_text, "latitude_text": coordinate.latitude_text,
        "resolution_arcminutes": coordinate.resolution_arcminutes, "datum": coordinate.datum,
        "reconstructed": false, "geometry_role": "source_observation"
    })
}

/// Registered map point with the same evidence properties in interactive and tiled maps.
pub fn map_location_feature(
    estimate: &Estimate,
    record: &Record,
    entity: &Entity,
) -> Option<Value> {
    let EstimateValue::MapLocation {
        record_id,
        point,
        datum,
        uncertainty_m,
        registration,
    } = &estimate.value
    else {
        return None;
    };
    Some(
        json!({"type":"Feature","geometry":{"type":"Point","coordinates":[point.longitude,point.latitude]},
        "properties":{"entity_id":entity.id,"entity_kind":entity.kind,
            "entity_domain":entity.kind.domain(),"entity_subkind":entity.kind.subkind(),
            "estimate_id":estimate.id,"record_id":record_id,
            "name":if record.observation.name.is_empty() { &entity.key } else { &record.observation.name },
            "source_id":record.source_id,"page_or_sheet":record.page_or_sheet,"scan_page":record.scan_page,
            "geometry_role":"location_estimate","datum":datum,"uncertainty_m":uncertainty_m,
            "basis":estimate.basis,"brief":estimate.brief,"registration":registration,"reconstructed":false}}),
    )
}
