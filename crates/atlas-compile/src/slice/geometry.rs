use super::SliceError;
use crate::export::{
    Artifacts, ExportError,
    chunks::{ChunkKind, insert_chunks},
};

use atlas_core::{
    identity::EntityId,
    time::{EpochTime, TemporalSpan},
};
use atlas_ledger::{model::Dataset, timeline::TimeSelection};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn publish(
    output: &mut Artifacts,
    data: &Dataset,
    at: EpochTime,
    selection: TimeSelection,
    absent: &BTreeSet<EntityId>,
) -> Result<Vec<String>, SliceError> {
    let mut features = Vec::new();
    for mut feature in crate::export::geometry::features(output, data)? {
        let span: Option<TemporalSpan> = serde_json::from_value(
            feature
                .pointer("/properties/validity")
                .cloned()
                .unwrap_or_default(),
        )
        .map_err(ExportError::from)?;
        let id: EntityId = serde_json::from_value(
            feature
                .pointer("/properties/entity_id")
                .cloned()
                .unwrap_or_default(),
        )
        .map_err(ExportError::from)?;
        if !absent.contains(&id)
            && let Some(membership) = selection.select(span.as_ref(), at)?
        {
            if let Some(properties) = feature.get_mut("properties").and_then(Value::as_object_mut) {
                properties.insert("membership".into(), serde_json::json!(membership));
            }
            features.push(feature);
        }
    }
    Ok(insert_chunks(
        output,
        "slice/geometry",
        &features,
        ChunkKind::GeoJson,
    )?)
}

#[cfg(test)]
mod tests;
