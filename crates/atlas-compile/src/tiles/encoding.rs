use geozero::mvt::{Message, TagsBuilder, Tile, tile};

use super::{
    features::ObservationPoint,
    projection::{EXTENT, Position},
};

pub(super) fn encode(points: &[(&ObservationPoint<'_>, Position)]) -> Vec<u8> {
    let mut dictionary = TagsBuilder::new();
    let mut features = Vec::new();
    for (point, position) in points {
        let mut tags = Vec::new();
        for (key, value) in &point.properties {
            let (key_index, value_index) = dictionary.insert_ref(key, value.clone());
            tags.extend([key_index, value_index]);
        }
        features.push(tile::Feature {
            id: None,
            tags,
            r#type: Some(tile::GeomType::Point as i32),
            geometry: vec![9, zigzag(position.x), zigzag(position.y)],
        });
    }
    let (keys, values) = dictionary.into_tags();
    Tile {
        layers: vec![tile::Layer {
            version: 2,
            name: "observations".into(),
            extent: Some(EXTENT),
            keys,
            values: values.into_iter().map(Into::into).collect(),
            features,
        }],
    }
    .encode_to_vec()
}

fn zigzag(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}
