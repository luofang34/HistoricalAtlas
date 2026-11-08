use std::collections::BTreeMap;

use atlas_geo::{body::BodyId, coordinates::CoordinateObservation};
use atlas_ledger::model::{Entity, Record};
use geozero::mvt::TileValue;
use serde_json::json;

use crate::input::LedgerInput;

use super::{TileError, projection};

pub(super) struct ObservationPoint<'a> {
    pub coordinate: &'a CoordinateObservation,
    pub projected: (f64, f64),
    pub properties: BTreeMap<String, TileValue>,
}

pub(super) fn collect<'a>(
    input: &'a LedgerInput,
    body: BodyId,
    base: &str,
) -> Result<Vec<ObservationPoint<'a>>, TileError> {
    let entities: BTreeMap<_, _> = input
        .data
        .entities
        .iter()
        .filter(|entity| entity.body == body)
        .map(|entity| (entity.id, entity))
        .collect();
    let mut records: Vec<_> = input.data.records.iter().filter(|r| r.checked).collect();
    records.sort_by_key(|record| record.id);
    let mut result = Vec::new();
    for record in records {
        if let Some(entity) = entities.get(&record.entity_id)
            && let Some(coordinate) = &record.observation.coordinate
        {
            result.push(ObservationPoint {
                coordinate,
                projected: projection::project(coordinate.point, record.id)?,
                properties: properties(record, entity, coordinate, base)?,
            });
        }
    }
    Ok(result)
}

fn properties(
    record: &Record,
    entity: &Entity,
    coordinate: &CoordinateObservation,
    base: &str,
) -> Result<BTreeMap<String, TileValue>, TileError> {
    let source_date = record
        .source_date
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let values = json!({
        "entity_id": entity.id,
        "record_id": record.id,
        "entity_kind": entity.kind,
        "name": record.observation.name,
        "pinyin": record.observation.pinyin,
        "population": record.observation.population,
        "households": record.observation.households,
        "rank": record.observation.rank,
        "parent_id": record.observation.parent_id,
        "seat_id": record.observation.seat_id,
        "source_id": record.source_id,
        "page_or_sheet": record.page_or_sheet,
        "scan_page": record.scan_page,
        "locator": record.locator,
        "source_date": source_date,
        "longitude": coordinate.point.longitude,
        "latitude": coordinate.point.latitude,
        "longitude_text": coordinate.longitude_text,
        "latitude_text": coordinate.latitude_text,
        "resolution_arcminutes": coordinate.resolution_arcminutes,
        "datum": coordinate.datum,
        "reconstructed": false,
        "geometry_role": "source_observation",
        "evidence_url": format!("{base}entities/index/{}.json", entity.id.shard())
    });
    let mut result = BTreeMap::new();
    if let serde_json::Value::Object(fields) = values {
        for (key, value) in fields {
            let scalar = match value {
                serde_json::Value::String(text) => TileValue::Str(text),
                serde_json::Value::Bool(value) => TileValue::Bool(value),
                serde_json::Value::Number(value) => match value.as_u64() {
                    Some(integer) => TileValue::Uint(integer),
                    None => TileValue::Double(value.as_f64().unwrap_or_default()),
                },
                _ => continue,
            };
            result.insert(key, scalar);
        }
    }
    Ok(result)
}
