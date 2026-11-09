use crate::catalog::Atlas;
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use atlas_ledger::model::EstimateValue;
use serde_json::{Value, json};

pub(super) fn features(atlas: &Atlas, body: BodyId, selected: Option<EntityId>) -> Value {
    let features: Vec<_> = atlas
        .data
        .estimates
        .iter()
        .filter_map(|estimate| {
            let entry = atlas.entry(estimate.entity_id)?;
            let EstimateValue::MapLocation { record_id, .. } = estimate.value else {
                return None;
            };
            if entry.entity.body != body {
                return None;
            }
            let record = entry.records.iter().find(|r| r.id == record_id)?;
            let mut feature =
                atlas_ledger::features::map_location_feature(estimate, record, &entry.entity)?;
            if let Some(properties) = feature.get_mut("properties").and_then(Value::as_object_mut) {
                properties.insert(
                    "selected".into(),
                    json!(selected == Some(estimate.entity_id)),
                );
            }
            Some(feature)
        })
        .collect();
    json!({"type": "FeatureCollection", "features": features})
}
