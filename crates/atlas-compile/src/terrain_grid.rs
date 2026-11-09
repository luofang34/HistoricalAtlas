//! Crop fingerprinted Web Mercator elevation tiles into an evidence-linked terrain grid.

use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use atlas_boundary::terrain::ElevationGrid;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tiff::{decoder::DecodingResult, tags::Tag};

use crate::{
    derivation::TerrainTile, export::MAX_FILE_BYTES, input::read_json_blocking,
    registration::RegistrationTerrain,
};

/// Half the Web Mercator world width in metres.
const HALF_WORLD_M: f64 = 20_037_508.342_789_244;

/// Tiles and bounds for one terrain grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainGridRequest {
    /// Catalogued elevation source.
    pub source_id: String,
    /// Longitude and latitude bounds: west, south, east, north.
    pub bounds: [f64; 4],
    /// Downloaded GeoTIFF tiles and their public URLs.
    pub tiles: Vec<LocalTile>,
    /// Height of open water, when the tiles include seafloor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_surface_m: Option<f64>,
}

/// A downloaded tile and where it was published.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalTile {
    /// Public source-tile URL.
    pub url: String,
    /// Local GeoTIFF path, relative to the request file.
    pub path: PathBuf,
}

struct Raster {
    west_m: f64,
    north_m: f64,
    cell_m: f64,
    width: usize,
    height: usize,
    samples: Vec<i16>,
}

/// Read a request, crop its tiles, and write a new terrain file.
pub fn terrain_grid_blocking(request: &Path, output: &Path) -> Result<usize, TerrainGridError> {
    let parsed: TerrainGridRequest =
        read_json_blocking(request).map_err(|error| TerrainGridError::Input(error.to_string()))?;
    let base = request.parent().unwrap_or(Path::new("."));
    let mut tiles = Vec::new();
    let mut rasters = Vec::new();
    for tile in &parsed.tiles {
        let path = base.join(&tile.path);
        let bytes = std::fs::read(&path).map_err(|source| TerrainGridError::Io {
            path: path.clone(),
            source,
        })?;
        tiles.push(TerrainTile {
            url: tile.url.clone(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        });
        rasters.push(read_tile(&path)?);
    }
    let terrain = RegistrationTerrain {
        source_id: parsed.source_id,
        tiles,
        grid: crop(&rasters, parsed.bounds)?,
        water_surface_m: parsed.water_surface_m,
    };
    let bytes =
        serde_json::to_vec(&terrain).map_err(|error| TerrainGridError::Input(error.to_string()))?;
    if bytes.len() >= MAX_FILE_BYTES {
        return Err(TerrainGridError::Input(format!(
            "terrain grid is {} bytes, exceeding the static file budget",
            bytes.len()
        )));
    }
    let io = |source| TerrainGridError::Io {
        path: output.to_owned(),
        source,
    };
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(io)?
        .write_all(&bytes)
        .map_err(io)?;
    Ok(terrain.grid.elevations_m.len())
}

fn read_tile(path: &Path) -> Result<Raster, TerrainGridError> {
    let invalid = |message: &str| TerrainGridError::Tile {
        path: path.to_owned(),
        message: message.into(),
    };
    let file = File::open(path).map_err(|source| TerrainGridError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut decoder =
        tiff::decoder::Decoder::new(BufReader::new(file)).map_err(|e| invalid(&e.to_string()))?;
    let (width, height) = decoder.dimensions().map_err(|e| invalid(&e.to_string()))?;
    let scale = decoder
        .get_tag_f64_vec(Tag::ModelPixelScaleTag)
        .map_err(|e| invalid(&e.to_string()))?;
    let tiepoint = decoder
        .get_tag_f64_vec(Tag::ModelTiepointTag)
        .map_err(|e| invalid(&e.to_string()))?;
    let (Some(&cell_m), Some(&west_m), Some(&north_m)) =
        (scale.first(), tiepoint.get(3), tiepoint.get(4))
    else {
        return Err(invalid("missing GeoTIFF pixel scale or tiepoint"));
    };
    let samples = match decoder.read_image().map_err(|e| invalid(&e.to_string()))? {
        DecodingResult::I16(values) => values,
        _ => return Err(invalid("expected signed 16-bit elevation samples")),
    };
    if samples.contains(&i16::MIN) || !(cell_m.is_finite() && cell_m > 0.0) {
        return Err(invalid("void elevation sample or invalid georeferencing"));
    }
    Ok(Raster {
        west_m,
        north_m,
        cell_m,
        width: width as usize,
        height: height as usize,
        samples,
    })
}

/// Cells of the tiles' common grid whose centres fall inside the bounds.
fn crop(rasters: &[Raster], bounds: [f64; 4]) -> Result<ElevationGrid, TerrainGridError> {
    let first = rasters
        .first()
        .ok_or_else(|| TerrainGridError::Input("no terrain tiles supplied".into()))?;
    let cell_m = first.cell_m;
    if rasters
        .iter()
        .any(|raster| (raster.cell_m - cell_m).abs() > cell_m * 1e-9)
    {
        return Err(TerrainGridError::Input("tiles differ in resolution".into()));
    }
    let [west, south, east, north] = bounds;
    let x = |longitude: f64| longitude.to_radians() * HALF_WORLD_M / std::f64::consts::PI;
    let y =
        |latitude: f64| latitude.to_radians().tan().asinh() * HALF_WORLD_M / std::f64::consts::PI;
    let origin_x = -HALF_WORLD_M;
    let origin_y = HALF_WORLD_M;
    let first_column = ((x(west) - origin_x) / cell_m).floor();
    let last_column = ((x(east) - origin_x) / cell_m).ceil();
    let first_row = ((origin_y - y(north)) / cell_m).floor();
    let last_row = ((origin_y - y(south)) / cell_m).ceil();
    let width = (last_column - first_column) as usize;
    let height = (last_row - first_row) as usize;
    if !(west < east && south < north) || width == 0 || height == 0 {
        return Err(TerrainGridError::Input("empty terrain bounds".into()));
    }
    let west_m = origin_x + first_column * cell_m;
    let north_m = origin_y - first_row * cell_m;
    let mut elevations_m = Vec::with_capacity(width * height);
    for row in 0..height {
        for column in 0..width {
            let centre_x = west_m + (column as f64 + 0.5) * cell_m;
            let centre_y = north_m - (row as f64 + 0.5) * cell_m;
            let sample = rasters
                .iter()
                .find_map(|raster| raster.sample(centre_x, centre_y));
            elevations_m.push(sample.ok_or_else(|| {
                TerrainGridError::Input(format!(
                    "no tile covers Web Mercator cell centre ({centre_x:.1}, {centre_y:.1})"
                ))
            })?);
        }
    }
    let grid = ElevationGrid {
        width,
        height,
        west_m,
        north_m,
        cell_m,
        elevations_m,
    };
    grid.validate()
        .map_err(|error| TerrainGridError::Input(error.to_string()))?;
    Ok(grid)
}

impl Raster {
    fn sample(&self, x: f64, y: f64) -> Option<i16> {
        let column = ((x - self.west_m) / self.cell_m).floor();
        let row = ((self.north_m - y) / self.cell_m).floor();
        if column < 0.0 || row < 0.0 || column >= self.width as f64 || row >= self.height as f64 {
            return None;
        }
        self.samples
            .get(row as usize * self.width + column as usize)
            .copied()
    }
}

/// Terrain request, tile, or output failure.
#[derive(Debug, Error)]
pub enum TerrainGridError {
    /// Invalid request or bounds.
    #[error("terrain grid: {0}")]
    Input(String),
    /// Unreadable or unsuitable GeoTIFF.
    #[error("terrain tile {path}: {message}")]
    Tile {
        /// Tile path.
        path: PathBuf,
        /// Decoder or validation message.
        message: String,
    },
    /// File could not be read or written.
    #[error("terrain grid file {path}: {source}")]
    Io {
        /// File path.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests;
