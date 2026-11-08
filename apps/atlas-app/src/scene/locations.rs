use crate::catalog::Atlas;
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use atlas_ledger::model::EstimateValue;
use serde_json::{Value, json};

pub(super) fn features(atlas: &Atlas, body: BodyId, selected: Option<EntityId>) -> Value {
    let features: Vec<_> = atlas.data.estimates.iter().filter_map(|estimate| {
        let entry = atlas.entry(estimate.entity_id)?;
        let EstimateValue::MapLocation { record_id, point, datum, uncertainty_m, .. } = estimate.value else { return None; };
        if entry.entity.body != body { return None; }
        let record = entry.records.iter().find(|r| r.id == record_id)?;
        Some(json!({"type": "Feature", "geometry": {"type": "Point", "coordinates": [point.longitude, point.latitude]},
            "properties": {"entity_id": estimate.entity_id, "estimate_id": estimate.id,
                "name": record.observation.name, "record_id": record_id, "selected": selected == Some(estimate.entity_id),
                "geometry_role": "location_estimate", "datum": datum, "uncertainty_m": uncertainty_m,
                "basis": estimate.basis, "brief": estimate.brief}}))
    }).collect();
    json!({"type": "FeatureCollection", "features": features})
}
