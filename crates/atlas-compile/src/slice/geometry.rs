use super::SliceError;
use crate::export::{
    Artifacts, ExportError,
    chunks::{ChunkKind, insert_chunks},
};
use atlas_boundary::{geographic::GeographicBoundary, terrain::DerivedBoundary};
use atlas_core::{
    identity::EntityId,
    time::{EpochTime, TemporalSpan},
};
use atlas_ledger::{model::Dataset, timeline::TimeSelection};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn publish(
    output: &mut Artifacts,
    data: &Dataset,
    at: EpochTime,
    selection: TimeSelection,
    absent: &BTreeSet<EntityId>,
) -> Result<Vec<String>, SliceError> {
    let mut features = Vec::new();
    for (_, bytes) in output
        .iter()
        .filter(|(path, _)| path.starts_with("entities/index"))
    {
        let entities: BTreeMap<String, Value> =
            serde_json::from_slice(bytes).map_err(ExportError::from)?;
        for entry in entities.values() {
            for key in ["boundaries", "derived_boundaries"] {
                for boundary in entry[key].as_array().into_iter().flatten() {
                    let mut feature = if key == "boundaries" {
                        serde_json::from_value::<GeographicBoundary>(boundary.clone())
                            .map_err(ExportError::from)?
                            .feature(data, None)
                            .map_err(ExportError::from)?
                    } else {
                        serde_json::from_value::<DerivedBoundary>(boundary.clone())
                            .map_err(ExportError::from)?
                            .feature(data, None)
                            .map_err(ExportError::from)?
                    };
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
                        if let Some(properties) =
                            feature.get_mut("properties").and_then(Value::as_object_mut)
                        {
                            properties.insert("membership".into(), serde_json::json!(membership));
                        }
                        features.push(feature);
                    }
                }
            }
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
