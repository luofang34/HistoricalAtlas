//! Bundled physical geography, shared by flat and globe rendering.

use maplibre::{
    coords::{WorldTileCoords, ZoomLevel},
    raster::{AvailableRasterLayerData, RasterSourceId},
};

const WORLD: &[u8] = include_bytes!("../../assets/basemap/natural-earth-0-0-0.jpg");
const TERRAIN: [(i32, i32, &[u8]); 4] = [
    (
        202,
        107,
        include_bytes!("../../assets/basemap/terrain-8-202-107.jpg"),
    ),
    (
        202,
        108,
        include_bytes!("../../assets/basemap/terrain-8-202-108.jpg"),
    ),
    (
        203,
        107,
        include_bytes!("../../assets/basemap/terrain-8-203-107.jpg"),
    ),
    (
        203,
        108,
        include_bytes!("../../assets/basemap/terrain-8-203-108.jpg"),
    ),
];

/// Decode locally bundled tiles; both renderers use the same geographic coverage.
pub fn tiles() -> Result<Vec<AvailableRasterLayerData>, image::ImageError> {
    let mut tiles = vec![tile("natural-earth", 0, 0, 0, WORLD)?];
    for (x, y, bytes) in TERRAIN {
        tiles.push(tile("terrain", 8, x, y, bytes)?);
    }
    Ok(tiles)
}

fn tile(
    source: &str,
    z: u8,
    x: i32,
    y: i32,
    bytes: &[u8],
) -> Result<AvailableRasterLayerData, image::ImageError> {
    Ok(AvailableRasterLayerData {
        coords: WorldTileCoords {
            x,
            y,
            z: ZoomLevel::new(z),
        },
        source: RasterSourceId::new(Some(source.into())),
        image: image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)?.into_rgba8(),
    })
}

#[cfg(test)]
mod tests;
