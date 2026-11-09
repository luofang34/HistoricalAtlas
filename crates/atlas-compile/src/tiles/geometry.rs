use super::{
    TileError, features,
    projection::Address,
    shapes::{Shape, commands},
};
use crate::{
    export::{Artifacts, insert, insert_bytes},
    input::LedgerInput,
};
use atlas_core::identity::{EntityId, FactId};
use atlas_ledger::model::EstimateValue;
use geozero::mvt::{Message, TagsBuilder, Tile, TileValue, tile};
use serde_json::{Value, json};
use std::collections::BTreeMap;

mod metadata;

struct Feature {
    layer: &'static str,
    shape: Shape,
    properties: BTreeMap<String, TileValue>,
}

pub(super) fn publish(
    input: &LedgerInput,
    output: &mut Artifacts,
    base: &str,
    maxzoom: u8,
) -> Result<(), TileError> {
    let mut features = crate::export::geometry::features(output, &input.data)?;
    for estimate in &input.data.estimates {
        if let EstimateValue::MapLocation { record_id, .. } = estimate.value
            && let Some(record) = input.data.records.iter().find(|r| r.id == record_id)
            && let Some(entity) = input
                .data
                .entities
                .iter()
                .find(|e| e.id == estimate.entity_id)
            && let Some(feature) =
                atlas_ledger::features::map_location_feature(estimate, record, entity)
        {
            features.push(feature);
        }
    }
    let mut bodies = BTreeMap::<String, Vec<Feature>>::new();
    for feature in features {
        let id: EntityId = serde_json::from_value(
            feature
                .pointer("/properties/entity_id")
                .cloned()
                .unwrap_or_default(),
        )?;
        let entity = input
            .data
            .entities
            .iter()
            .find(|e| e.id == id)
            .ok_or(TileError::Geometry("missing feature subject"))?;
        let body = serde_json::to_value(entity.body)?
            .as_str()
            .ok_or(TileError::Geometry("invalid body"))?
            .to_owned();
        bodies
            .entry(body)
            .or_default()
            .push(prepare(feature, base)?);
    }
    let mut inventory = Vec::new();
    for (body, features) in bodies {
        let tiles = tile_features(&features, maxzoom)?;
        let addresses: Vec<_> = tiles.keys().copied().collect();
        for (address, layers) in tiles {
            insert_bytes(
                output,
                &format!(
                    "tiles/{body}/geometry/{}/{}/{}.mvt",
                    address.z, address.x, address.y
                ),
                encode(layers),
            )?;
        }
        insert(
            output,
            &format!("tiles/{body}/geometry/index.json"),
            &addresses,
        )?;
        metadata::publish(output, base, &body, maxzoom, &features)?;
        inventory.push(json!({"body":body,"tilejson":format!("{base}tiles/{body}/geometry.json"),"features":features.len()}));
    }
    insert(output, "tiles/geometry.json", &inventory)?;
    Ok(())
}

fn prepare(feature: Value, base: &str) -> Result<Feature, TileError> {
    let fact: FactId = serde_json::from_value(
        feature
            .pointer("/properties/record_id")
            .cloned()
            .unwrap_or_default(),
    )?;
    let id: EntityId = serde_json::from_value(
        feature
            .pointer("/properties/entity_id")
            .cloned()
            .unwrap_or_default(),
    )?;
    let shape = Shape::from_json(feature.get("geometry").cloned().unwrap_or_default(), fact)?;
    let layer = match shape {
        Shape::Point(_) => "map_locations",
        Shape::Lines(_) => "lines",
        Shape::Polygons(_) => "boundaries",
    };
    let mut properties = feature
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .ok_or(TileError::Geometry("missing properties"))?;
    properties.insert(
        "evidence_url".into(),
        json!(format!("{base}entities/index/{}.json", id.shard())),
    );
    let span = properties.get("validity").cloned().unwrap_or_default();
    // Exact TT milliseconds stay decimal strings, including deep-time values beyond i64.
    for (key, end, estimate) in [
        ("valid_from", "start", "nominal"),
        ("valid_to", "end", "nominal"),
        ("possible_from", "start", "earliest"),
        ("possible_to", "end", "latest"),
        ("certain_from", "start", "latest"),
        ("certain_to", "end", "earliest"),
    ] {
        if let Some(value) = span.get(end).and_then(|v| v.get(estimate)) {
            properties.insert(key.into(), value.clone());
        }
    }
    Ok(Feature {
        layer,
        shape,
        properties: features::scalars(Value::Object(properties))?,
    })
}

type TileLayers = BTreeMap<&'static str, Vec<(BTreeMap<String, TileValue>, tile::Feature)>>;

fn tile_features(
    features: &[Feature],
    maxzoom: u8,
) -> Result<BTreeMap<Address, TileLayers>, TileError> {
    let mut tiles = BTreeMap::<_, TileLayers>::new();
    for feature in features {
        for zoom in 0..=maxzoom {
            for (address, shape) in feature.shape.tiles(zoom)? {
                let (kind, geometry) = commands(&shape);
                if geometry.is_empty() {
                    continue;
                }
                tiles
                    .entry(address)
                    .or_default()
                    .entry(feature.layer)
                    .or_default()
                    .push((
                        feature.properties.clone(),
                        tile::Feature {
                            id: None,
                            tags: Vec::new(),
                            r#type: Some(kind as i32),
                            geometry,
                        },
                    ));
            }
        }
    }
    Ok(tiles)
}

fn encode(layers: TileLayers) -> Vec<u8> {
    let layers = layers
        .into_iter()
        .map(|(name, features)| {
            let mut dictionary = TagsBuilder::new();
            let features = features
                .into_iter()
                .map(|(properties, mut feature)| {
                    for (key, value) in properties {
                        let (key, value) = dictionary.insert_ref(&key, value);
                        feature.tags.extend([key, value]);
                    }
                    feature
                })
                .collect();
            let (keys, values) = dictionary.into_tags();
            tile::Layer {
                version: 2,
                name: name.into(),
                extent: Some(super::projection::EXTENT),
                keys,
                values: values.into_iter().map(Into::into).collect(),
                features,
            }
        })
        .collect();
    Tile { layers }.encode_to_vec()
}

#[cfg(test)]
mod tests;
