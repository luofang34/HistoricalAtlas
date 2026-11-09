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
    let mut values = atlas_ledger::features::observation_properties(record, entity, coordinate);
    if let Some(values) = values.as_object_mut() {
        values.insert(
            "evidence_url".into(),
            json!(format!("{base}entities/index/{}.json", entity.id.shard())),
        );
    }
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
                serde_json::Value::Null => continue,
                value => TileValue::Str(serde_json::to_string(&value)?),
            };
            result.insert(key, scalar);
        }
    }
    Ok(result)
}
