use std::{collections::BTreeSet, path::PathBuf};

use geozero::mvt::{Message, Tile, TileValue, tile};

use super::*;

const BASE: &str = "https://atlas.example/project/";

#[test]
fn unresolved_printed_longitude_remains_in_evidence_without_becoming_a_tile_point() {
    let input = input();
    let record = input
        .data
        .records
        .iter()
        .find(|record| record.key == "bijie-gazetteer/p38/toubu/yinhe-brigade")
        .unwrap();
    let output = compile_tiles(&input, BASE, 0).unwrap();
    let layer = root_layer(&output, "earth");
    assert!(
        !layer
            .features
            .iter()
            .any(|feature| properties(&layer, feature)["record_id"]
                == text(&record.id.0.to_string()))
    );
    let path = PathBuf::from(format!("entities/index/{}.json", record.entity_id.shard()));
    let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
    let records = shard[record.entity_id.0.to_string()]["records"]
        .as_array()
        .unwrap();
    let published = records
        .iter()
        .find(|value| value["id"] == record.id.0.to_string())
        .unwrap();
    assert!(published["observation"]["coordinate"].is_null());
    let raw = &published["observation"]["unresolved_coordinate"];
    assert_eq!(raw["longitude_text"], "185°20′");
    assert_eq!(raw["latitude_text"], "27°17′");
    assert_eq!(raw["resolution_arcminutes"], 1.0);
    assert_eq!(published["scan_page"], 73);
    assert_eq!(published["page_or_sheet"], "38");
}

#[cfg(test)]
fn input() -> LedgerInput {
    load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

#[cfg(test)]
fn root_layer(output: &Artifacts, body: &str) -> tile::Layer {
    let path = PathBuf::from(format!("tiles/{body}/observations/0/0/0.mvt"));
    let tile = Tile::decode(output[&path].as_slice()).unwrap();
    assert_eq!(tile.layers.len(), 1);
    let layer = tile.layers.into_iter().next().unwrap();
    assert_eq!(layer.name, "observations");
    assert_eq!(layer.version, 2);
    assert_eq!(layer.extent, Some(4096));
    layer
}

#[cfg(test)]
fn properties(layer: &tile::Layer, feature: &tile::Feature) -> BTreeMap<String, TileValue> {
    assert_eq!(feature.tags.len() % 2, 0);
    feature
        .tags
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            (
                layer.keys[pair[0] as usize].clone(),
                TileValue::try_from(layer.values[pair[1] as usize].clone()).unwrap(),
            )
        })
        .collect()
}

#[cfg(test)]
fn text(value: &str) -> TileValue {
    TileValue::Str(value.into())
}

#[test]
fn actual_ledger_tiles_preserve_checked_coordinates_and_evidence_links() {
    let input = input();
    let output = compile_tiles(&input, BASE, 2).unwrap();
    assert!(
        output
            .values()
            .all(|bytes| bytes.len() < crate::export::MAX_FILE_BYTES)
    );
    let mut found = BTreeSet::new();
    for body in ["earth", "moon"] {
        let layer = root_layer(&output, body);
        for feature in &layer.features {
            assert_eq!(feature.r#type, Some(tile::GeomType::Point as i32));
            assert_eq!(feature.geometry.len(), 3);
            assert_eq!(feature.geometry[0], 9);
            let properties = properties(&layer, feature);
            let record = input
                .data
                .records
                .iter()
                .find(|record| properties["record_id"] == text(&record.id.0.to_string()))
                .unwrap();
            assert!(record.checked);
            assert!(found.insert(record.id));
            let coordinate = record.observation.coordinate.as_ref().unwrap();
            assert_eq!(
                properties["entity_id"],
                text(&record.entity_id.0.to_string())
            );
            assert_eq!(properties["name"], text(&record.observation.name));
            assert_eq!(
                properties["longitude_text"],
                text(&coordinate.longitude_text)
            );
            assert_eq!(properties["latitude_text"], text(&coordinate.latitude_text));
            assert_eq!(
                properties["resolution_arcminutes"],
                TileValue::Double(coordinate.resolution_arcminutes)
            );
            assert_eq!(properties["reconstructed"], TileValue::Bool(false));
            assert!(!properties.contains_key("valid_from"));
            assert!(!properties.contains_key("valid_to"));
            let TileValue::Str(url) = &properties["evidence_url"] else {
                panic!("evidence URL")
            };
            let evidence: serde_json::Value =
                serde_json::from_slice(&output[&PathBuf::from(url.strip_prefix(BASE).unwrap())])
                    .unwrap();
            assert!(
                evidence[record.entity_id.0.to_string()]["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value["id"] == record.id.0.to_string())
            );
        }
    }
    let expected: BTreeSet<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.checked && record.observation.coordinate.is_some())
        .map(|record| record.id)
        .collect();
    assert_eq!(found, expected);
    assert!(!output.contains_key(Path::new("tiles/mars/observations.json")));
}

#[test]
fn coincident_villages_and_printed_anomalies_survive_mvt_encoding() {
    let output = compile_tiles(&input(), BASE, 0).unwrap();
    let layer = root_layer(&output, "earth");
    let find = |name: &str| {
        layer
            .features
            .iter()
            .find(|feature| properties(&layer, feature)["name"] == text(name))
            .unwrap()
    };
    let first = find("厦耳");
    let second = find("崩土山");
    assert_eq!(first.geometry, second.geometry);
    assert_ne!(
        properties(&layer, first)["entity_id"],
        properties(&layer, second)["entity_id"]
    );
    let anomalous = properties(&layer, find("吴家沟"));
    assert_eq!(anomalous["latitude_text"], text("20°16′"));
    assert_eq!(anomalous["datum"], text("unspecified"));
    assert!(!properties(&layer, find("黄家大院")).contains_key("population"));
    assert_eq!(
        properties(&layer, find("坡坪子"))["population"],
        TileValue::Uint(15)
    );
}

#[test]
fn tilejson_styles_and_inventory_resolve_under_the_publication_directory() {
    let output = compile_tiles(&input(), BASE.trim_end_matches('/'), 3).unwrap();
    let bodies: serde_json::Value =
        serde_json::from_slice(&output[Path::new("tiles/index.json")]).unwrap();
    for body in bodies.as_array().unwrap() {
        let url = body["tilejson"].as_str().unwrap();
        let path = PathBuf::from(url.strip_prefix(BASE).unwrap());
        let manifest: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
        assert_eq!(manifest["tilejson"], "3.0.0");
        assert_eq!(manifest["scheme"], "xyz");
        assert_eq!(manifest["minzoom"], 0);
        assert_eq!(manifest["maxzoom"], 3);
        assert_eq!(manifest["vector_layers"][0]["id"], "observations");
        assert_eq!(manifest["atlas:body"], body["body"]);
        let inventory_url = manifest["atlas:availability"].as_str().unwrap();
        let inventory: serde_json::Value = serde_json::from_slice(
            &output[&PathBuf::from(inventory_url.strip_prefix(BASE).unwrap())],
        )
        .unwrap();
        let template = manifest["tiles"][0].as_str().unwrap();
        let mut zooms = BTreeSet::new();
        for entry in inventory.as_array().unwrap() {
            let url = template
                .replace("{z}", &entry["z"].to_string())
                .replace("{x}", &entry["x"].to_string())
                .replace("{y}", &entry["y"].to_string());
            let tile =
                Tile::decode(output[&PathBuf::from(url.strip_prefix(BASE).unwrap())].as_slice())
                    .unwrap();
            assert!(!tile.layers[0].features.is_empty());
            zooms.insert(entry["z"].as_u64().unwrap());
        }
        assert_eq!(zooms, BTreeSet::from([0, 1, 2, 3]));
        let style_url = body["style"].as_str().unwrap();
        let style: serde_json::Value =
            serde_json::from_slice(&output[&PathBuf::from(style_url.strip_prefix(BASE).unwrap())])
                .unwrap();
        assert_eq!(style["version"], 8);
        assert_eq!(style["sources"]["observations"]["url"], body["tilejson"]);
    }
}

#[test]
fn shuffled_input_produces_identical_tiles_and_manifests() {
    let mut input = input();
    let expected = compile_tiles(&input, BASE, 3).unwrap();
    input.data.entities.reverse();
    input.data.records.reverse();
    input.catalog.sources.reverse();
    assert_eq!(expected, compile_tiles(&input, BASE, 3).unwrap());
}

#[cfg(test)]
fn set_single_coordinate(input: &mut LedgerInput, longitude: f64, latitude: f64) {
    let target = input
        .data
        .records
        .iter()
        .find(|record| {
            record.source_id == "anshun-county-gazetteer"
                && record.checked
                && record.observation.coordinate.is_some()
        })
        .unwrap()
        .id;
    for record in &mut input.data.records {
        if record.id == target {
            let coordinate = record.observation.coordinate.as_mut().unwrap();
            coordinate.point = atlas_geo::coordinates::LonLat {
                longitude,
                latitude,
            };
        } else {
            record.observation.coordinate = None;
        }
    }
    input.data.estimates.retain(|estimate| {
        !matches!(
            estimate.value,
            atlas_ledger::model::EstimateValue::Plate { .. }
        )
    });
}

#[test]
fn equator_tile_edges_and_antimeridian_receive_buffered_copies() {
    let mut input = input();
    set_single_coordinate(&mut input, 0.0, 0.0);
    let output = compile_tiles(&input, BASE, 1).unwrap();
    let root = root_layer(&output, "earth");
    assert_eq!(root.features[0].geometry, [9, 4096, 4096]);
    for (x, y, geometry) in [
        (0, 0, [9, 8192, 8192]),
        (0, 1, [9, 8192, 0]),
        (1, 0, [9, 0, 8192]),
        (1, 1, [9, 0, 0]),
    ] {
        let path = PathBuf::from(format!("tiles/earth/observations/1/{x}/{y}.mvt"));
        let tile = Tile::decode(output[&path].as_slice()).unwrap();
        assert_eq!(tile.layers[0].features[0].geometry, geometry);
    }
    set_single_coordinate(&mut input, 180.0, 0.0);
    let positive = compile_tiles(&input, BASE, 1).unwrap();
    set_single_coordinate(&mut input, -180.0, 0.0);
    let negative = compile_tiles(&input, BASE, 1).unwrap();
    for x in [0, 1] {
        let path = PathBuf::from(format!("tiles/earth/observations/1/{x}/0.mvt"));
        let east = Tile::decode(positive[&path].as_slice()).unwrap();
        let west = Tile::decode(negative[&path].as_slice()).unwrap();
        assert_eq!(
            east.layers[0].features[0].geometry,
            west.layers[0].features[0].geometry
        );
    }
}

#[test]
fn known_mercator_parallel_and_negative_buffer_offsets_decode_correctly() {
    let mut input = input();
    set_single_coordinate(&mut input, 45.0, 66.513_260_443_111_86);
    let output = compile_tiles(&input, BASE, 0).unwrap();
    assert_eq!(
        root_layer(&output, "earth").features[0].geometry,
        [9, 5120, 2048]
    );
    set_single_coordinate(&mut input, -0.5, 0.0);
    let output = compile_tiles(&input, BASE, 1).unwrap();
    let tile =
        Tile::decode(output[Path::new("tiles/earth/observations/1/1/1.mvt")].as_slice()).unwrap();
    assert_eq!(tile.layers[0].features[0].geometry, [9, 21, 0]);
}

#[test]
fn invalid_configuration_polar_coordinates_and_oversized_tiles_fail_explicitly() {
    let mut input = input();
    for base in [
        "/relative/",
        "file:///tmp/",
        "https://user@host/",
        "https://host/?q=1",
        "https://host/#section",
    ] {
        assert!(compile_tiles(&input, base, 0).is_err());
    }
    assert!(matches!(
        compile_tiles(&input, BASE, 31),
        Err(TileError::Zoom(31))
    ));
    let long_base = format!("https://host/{}/", "a".repeat(50_000));
    assert!(matches!(compile_tiles(&input, &long_base, 0),
        Err(TileError::Export(crate::export::ExportError::Oversize { path, .. })) if path.ends_with(".mvt")));
    set_single_coordinate(&mut input, 0.0, 90.0);
    assert!(matches!(
        compile_tiles(&input, BASE, 0),
        Err(TileError::PolarCoordinate { .. })
    ));
}
