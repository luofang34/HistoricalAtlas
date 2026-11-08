use super::*;
use atlas_geo::body::BodyId;
use maplibre::projection::ProjectionType;
use serde_json::json;

#[test]
fn bundled_physical_tiles_decode_and_cover_the_research_area() {
    let decoded = tiles().unwrap();
    assert_eq!(decoded.len(), 5);
    let world = decoded
        .iter()
        .find(|tile| tile.coords.z == ZoomLevel::new(0))
        .unwrap();
    assert_eq!(world.image.dimensions(), (2048, 2048));
    for tile in decoded
        .iter()
        .filter(|tile| tile.coords.z == ZoomLevel::new(8))
    {
        assert_eq!(tile.image.dimensions(), (512, 512));
        assert!(tile.image.pixels().all(|p| p.0[3] == 255));
        let darkest = tile.image.pixels().map(|p| p.0[0]).min().unwrap();
        let lightest = tile.image.pixels().map(|p| p.0[0]).max().unwrap();
        assert!(lightest - darkest > 40);
    }
    for (longitude, latitude) in [
        (105.3, 27.3),
        (105.77, 26.67),
        (105.48, 26.2),
        (105.93, 26.25),
    ] {
        let x = ((longitude + 180.0) / 360.0 * 256.0_f64).floor() as i32;
        let y = ((1.0 - f64::tan(f64::to_radians(latitude)).asinh() / std::f64::consts::PI) / 2.0
            * 256.0)
            .floor() as i32;
        assert!(decoded.iter().any(|tile| tile.coords.z == ZoomLevel::new(8)
            && tile.coords.x == x
            && tile.coords.y == y));
    }
}

#[test]
fn projections_share_source_coordinates_and_earth_basemap_is_body_scoped() {
    let points = json!({"type": "FeatureCollection", "features": [{"type": "Feature", "geometry": {"type": "Point", "coordinates": [105.3, 27.3]}, "properties": {"geometry_role": "source_observation"}}]});
    for projection in [ProjectionType::Mercator, ProjectionType::Globe] {
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
