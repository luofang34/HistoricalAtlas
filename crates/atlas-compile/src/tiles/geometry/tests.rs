use super::*;
use geo::{Area, BoundingRect, Geometry};
use geozero::ToGeo;

#[cfg(test)]
fn feature(geometry: Value) -> Value {
    json!({"type":"Feature","geometry":geometry,"properties":{
        "entity_id":EntityId::from_key("earth/test/route").unwrap(),
        "record_id":FactId::from_key("survey/route").unwrap(),
        "validity":{"start":{"earliest":"-99999999999999999999","nominal":"-99999999999999999998","latest":"-99999999999999999997"},
            "end":{"earliest":"10","nominal":"11","latest":"12"}}}})
}

#[test]
fn tile_geometry_clips_lines_and_keeps_holes_and_exact_temporal_bounds() {
    let polygon = prepare(
        feature(
            json!({"type":"Polygon","coordinates":[[[-30,-30],[30,-30],[30,30],[-30,30],[-30,-30]],
        [[-5,-5],[-5,5],[5,5],[5,-5],[-5,-5]]]}),
        ),
        "https://atlas.example/",
        &BTreeMap::new(),
    )
    .unwrap();
    let line = prepare(
        feature(json!({"type":"MultiLineString","coordinates":[[[-50,-20],[50,-20]],[[-50,20],[50,20]]]})),
        "https://atlas.example/",
        &BTreeMap::new(),
    )
    .unwrap();
    let tiles = tile_features(&[polygon, line], 2).unwrap();
    let root =
        Tile::decode(encode(tiles[&Address { z: 0, x: 0, y: 0 }].clone()).as_slice()).unwrap();
    let boundary = root
        .layers
        .iter()
        .find(|layer| layer.name == "boundaries")
        .unwrap();
    let polygon = match boundary.features[0].to_geo().unwrap() {
        Geometry::Polygon(polygon) => polygon,
        other => panic!("expected polygon, got {other:?}"),
    };
    assert_eq!(polygon.interiors().len(), 1);
    assert!(geo::Polygon::new(polygon.exterior().clone(), vec![]).signed_area() > 0.0);
    assert!(geo::Polygon::new(polygon.interiors()[0].clone(), vec![]).signed_area() < 0.0);
    let properties = super::super::tests::properties(boundary, &boundary.features[0]);
    assert_eq!(
        properties["valid_from"],
        TileValue::Str("-99999999999999999998".into())
    );
    assert_eq!(properties["possible_to"], TileValue::Str("12".into()));
    assert!(matches!(
        properties["possible_to_year"],
        TileValue::Double(_)
    ));
    let lines = root
        .layers
        .iter()
        .find(|layer| layer.name == "lines")
        .unwrap();
    let Geometry::MultiLineString(paths) = lines.features[0].to_geo().unwrap() else {
        panic!("expected lines")
    };
    assert_eq!(paths.0.len(), 2);
    assert!(paths.0.iter().all(|path| path.0.first() != path.0.last()));
    for (address, layers) in tiles {
        let tile = Tile::decode(encode(layers).as_slice()).unwrap();
        for feature in tile.layers.iter().flat_map(|layer| &layer.features) {
            let bounds = feature.to_geo().unwrap().bounding_rect().unwrap();
            assert!(
                bounds.min().x >= -64. && bounds.min().y >= -64.,
                "{address:?}"
            );
            assert!(
                bounds.max().x <= 4160. && bounds.max().y <= 4160.,
                "{address:?}"
            );
        }
    }
}

#[test]
fn quantization_omits_collapsed_geometry_and_limits_high_zoom_work() {
    let tiny = prepare(
        feature(json!({"type":"Polygon","coordinates":[[[1,1],[1.0000001,1],[1.0000001,1.0000001],[1,1]]]})),
        "https://atlas.example/",
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(tile_features(&[tiny], 0).unwrap().is_empty());
    let large = prepare(
        feature(json!({"type":"LineString","coordinates":[[-170,-70],[170,70]]})),
        "https://atlas.example/",
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(matches!(
        large.shape.tiles(30),
        Err(TileError::Coverage { .. })
    ));
}

#[test]
fn east_edge_line_stays_local_and_wraps_into_the_western_buffer() {
    let line = prepare(
        feature(json!({"type":"LineString","coordinates":[[179,1],[180,1]]})),
        "https://atlas.example/",
        &BTreeMap::new(),
    )
    .unwrap();
    let tiles = tile_features(&[line], 2).unwrap();
    let addresses: std::collections::BTreeSet<_> = tiles
        .keys()
        .filter(|address| address.z == 2)
        .map(|address| address.x)
        .collect();
    assert_eq!(addresses, std::collections::BTreeSet::from([0, 3]));
}
