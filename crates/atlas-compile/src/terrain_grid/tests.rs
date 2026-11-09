use super::*;
use tiff::encoder::{TiffEncoder, colortype::GrayI16};

/// Zoom-12 Web Mercator tile size in metres, as in Mapzen 512-pixel GeoTIFFs.
#[cfg(test)]
const TILE_M: f64 = 2.0 * HALF_WORLD_M / 4096.0;

#[cfg(test)]
fn write_tile(path: &Path, column: f64, row: f64, value: impl Fn(usize, usize) -> i16) {
    let size = 16;
    let cell = TILE_M / size as f64;
    let samples: Vec<i16> = (0..size * size)
        .map(|i| value(i / size, i % size))
        .collect();
    let mut encoder = TiffEncoder::new(File::create(path).unwrap()).unwrap();
    let mut image = encoder
        .new_image::<GrayI16>(size as u32, size as u32)
        .unwrap();
    image
        .encoder()
        .write_tag(Tag::ModelPixelScaleTag, &[cell, cell, 0.0][..])
        .unwrap();
    let west = -HALF_WORLD_M + column * TILE_M;
    let north = HALF_WORLD_M - row * TILE_M;
    image
        .encoder()
        .write_tag(
            Tag::ModelTiepointTag,
            &[0.0, 0.0, 0.0, west, north, 0.0][..],
        )
        .unwrap();
    image.write_data(&samples).unwrap();
}

#[test]
fn adjacent_tiles_crop_into_one_fingerprinted_grid_and_voids_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    // Tiles 3869 and 3870 of row 2155 at zoom 12 lie over Lunga Point.
    write_tile(&directory.path().join("a.tif"), 3869.0, 2155.0, |row, _| {
        row as i16
    });
    write_tile(&directory.path().join("b.tif"), 3870.0, 2155.0, |row, _| {
        100 + row as i16
    });
    let tile_degrees = 360.0 / 4096.0;
    let boundary = -180.0 + 3870.0 * tile_degrees;
    let request = TerrainGridRequest {
        source_id: "terrain".into(),
        bounds: [boundary - 0.01, -9.44, boundary + 0.01, -9.40],
        tiles: ["a", "b"]
            .map(|name| LocalTile {
                url: format!("https://example.org/{name}.tif"),
                path: format!("{name}.tif").into(),
            })
            .to_vec(),
        water_surface_m: Some(0.0),
    };
    let request_path = directory.path().join("request.json");
    std::fs::write(&request_path, serde_json::to_vec(&request).unwrap()).unwrap();
    let output = directory.path().join("terrain.json");
    let cells = terrain_grid_blocking(&request_path, &output).unwrap();
    let terrain: RegistrationTerrain =
        serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    assert_eq!(cells, terrain.grid.width * terrain.grid.height);
    assert_eq!(terrain.tiles.len(), 2);
    let bytes = std::fs::read(directory.path().join("a.tif")).unwrap();
    assert_eq!(
        terrain.tiles[0].sha256,
        format!("{:x}", Sha256::digest(&bytes))
    );
    let first_row = &terrain.grid.elevations_m[..terrain.grid.width];
    assert!(first_row.iter().any(|value| *value < 100));
    assert!(first_row.iter().any(|value| *value >= 100));
    assert!(terrain_grid_blocking(&request_path, &output).is_err());
    write_tile(&directory.path().join("b.tif"), 3870.0, 2155.0, |_, _| {
        i16::MIN
    });
    let error = terrain_grid_blocking(&request_path, &directory.path().join("void.json"));
    assert!(error.unwrap_err().to_string().contains("void"));
}
