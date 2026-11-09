use super::*;
use atlas_geo::body::BodyId;
use maplibre::{coords::WorldTileCoords, projection::ProjectionType};
use serde_json::json;

#[test]
fn publication_tiles_load_from_files_and_match_manifest_checksums() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let manifest = load().await.unwrap();
            let loader = loader(&manifest);
            for tile in &manifest.tiles {
                let url = format!("atlas://{}/{}/{}/{}", tile.source, tile.z, tile.x, tile.y);
                let bytes = loader.fetch(&url).await.unwrap();
                let image = image::load_from_memory(&bytes).unwrap().into_rgba8();
                assert!(image.pixels().all(|p| p.0[3] == 255));
                assert!(image.width() >= 256);
            }
            for url in [
                "atlas://terrain/0/0/0",
                "atlas://terrain/8/201/107",
                "atlas://../../secret",
            ] {
                assert!(loader.fetch(url).await.unwrap_err().is_not_found());
            }
            let mut corrupt = manifest.clone();
            corrupt.tiles[0].sha256 = "0".repeat(64);
            let tile = &corrupt.tiles[0];
            let url = format!("atlas://{}/{}/{}/{}", tile.source, tile.z, tile.x, tile.y);
            assert!(
                !super::loader(&corrupt)
                    .fetch(&url)
                    .await
                    .unwrap_err()
                    .is_retryable()
            );
        });
}

#[test]
fn projections_share_source_coordinates_and_earth_basemap_is_body_scoped() {
    let points = json!({"type": "FeatureCollection", "features": [{"type": "Feature", "geometry": {"type": "Point", "coordinates": [105.3, 27.3]}, "properties": {"geometry_role": "source_observation"}}]});
    for projection in [
        ProjectionType::Mercator,
        ProjectionType::VerticalPerspective,
    ] {
        let style = crate::scene::style(points.clone(), BodyId::Earth, projection.clone()).unwrap();
        assert!(style.validate().is_ok());
        assert_eq!(
            style.projection.as_ref().unwrap().projection_type,
            projection
        );
        let layers = style
            .layers
            .into_iter()
            .filter(|layer| layer.id == "observations")
            .collect();
        let processed = maplibre::headless::map::process_geojson_layers(
            &points,
            "observations",
            layers,
            WorldTileCoords::default(),
            projection,
        )
        .unwrap();
        assert!(!processed.vector.is_empty());
    }
    let moon = crate::scene::style(points, BodyId::Moon, ProjectionType::Globe).unwrap();
    let serialized = serde_json::to_value(moon).unwrap();
    for layer in serialized["layers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|layer| layer["type"] == "raster")
    {
        assert_eq!(layer["layout"]["visibility"], "none");
    }
}

#[test]
fn another_region_zoom_and_body_use_manifest_coverage() {
    let manifest = Manifest::parse(&serde_json::to_vec(&json!({"attribution": "Test imagery", "tiles": [
        {"source": "pacific", "z": 8, "x": 241, "y": 134, "file": "pacific.jpg", "sha256": "0".repeat(64)},
        {"source": "moon", "body": "moon", "z": 3, "x": 4, "y": 2, "file": "moon.jpg", "sha256": "0".repeat(64)}
    ]})).unwrap()).unwrap();
    let empty = json!({"type": "FeatureCollection", "features": []});
    let style =
        crate::scene::style_with_basemap(empty, BodyId::Moon, ProjectionType::Globe, &manifest)
            .unwrap();
    assert!(style.validate().is_ok());
    let style = serde_json::to_value(style).unwrap();
    let source = &style["sources"]["basemap-pacific-8"];
    assert_eq!(source["minzoom"], 8);
    assert_eq!(source["maxzoom"], 8);
    let bounds = source["bounds"].as_array().unwrap();
    assert!(bounds[0].as_f64().unwrap() < 160. && bounds[2].as_f64().unwrap() > 160.);
    assert!(bounds[1].as_f64().unwrap() < -9. && bounds[3].as_f64().unwrap() > -9.);
    let layers = style["layers"].as_array().unwrap();
    assert_eq!(
        layers
            .iter()
            .find(|layer| layer["id"] == "basemap-moon-3")
            .unwrap()["layout"]["visibility"]
            .as_str()
            .unwrap_or("visible"),
        "visible"
    );
    assert_eq!(
        layers
            .iter()
            .find(|layer| layer["id"] == "basemap-pacific-8")
            .unwrap()["layout"]["visibility"],
        "none"
    );
}

#[test]
fn invalid_tile_paths_addresses_and_duplicates_are_rejected() {
    let valid = serde_json::to_value(
        serde_json::from_slice::<serde_json::Value>(include_bytes!(
            "../../../assets/basemap/manifest.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for (field, value) in [
        ("file", json!("../tile.jpg")),
        ("z", json!(32)),
        ("x", json!(999)),
        ("source", json!("../terrain")),
        ("sha256", json!("invalid")),
    ] {
        let mut bad = valid.clone();
        bad["tiles"][0][field] = value;
        assert!(
            Manifest::parse(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{field}"
        );
    }
    let mut duplicate = valid;
    let tile = duplicate["tiles"][0].clone();
    duplicate["tiles"].as_array_mut().unwrap().push(tile);
    assert!(Manifest::parse(&serde_json::to_vec(&duplicate).unwrap()).is_err());
}
