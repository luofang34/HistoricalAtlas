//! Terrain-cost partitions clipped to an enclosing administrative polygon.

use atlas_geo::{coordinates::LonLat, registration::PlanarPoint};
use geo::{Contains, MapCoords, MultiPolygon, Point};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::polygon::{IndexedPolygon, resolve};

mod adjacency;
mod boundary;
mod cells;
mod connectivity;
mod fitting;
mod partition;
mod review;
mod surface;
mod vectorize;
pub use boundary::DerivedBoundary;
pub use cells::ElevationGrid;
pub use fitting::AreaTarget;
pub use review::{BoundaryComparison, NeighborComparison, NeighborExclusion, compare};

/// One located member of a numbered administrative unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainSeed {
    /// Index into the caller's ordered unit list.
    pub unit: usize,
    /// Angular coordinate aligned with the elevation grid.
    pub point: LonLat,
}

/// Numeric input for a reproducible terrain-cost partition.
#[derive(Debug, Clone)]
pub struct TerrainRequest {
    /// Web Mercator elevation cells in metres.
    pub grid: ElevationGrid,
    /// Longitude/latitude vertices enclosing the whole partition.
    pub vertices: Vec<PlanarPoint>,
    /// Indexed enclosing polygon components and holes.
    pub envelope: Vec<IndexedPolygon>,
    /// Located members with at least one seed per unit.
    pub seeds: Vec<TerrainSeed>,
    /// Number of units, indexed consecutively from zero.
    pub units: usize,
    /// Extra horizontal-equivalent metres per metre of elevation change.
    pub slope_weight: f64,
    /// Source areas to fit while retaining every seed's unit membership.
    pub areas: Vec<AreaTarget>,
    /// Unit pairs required to share an edge while retaining their located members.
    pub neighbors: Vec<[usize; 2]>,
}

/// A computed unit's vector geometry and occupied grid cells.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainRegion {
    /// Index of the unit in the input list.
    pub unit: usize,
    /// Longitude/latitude vertices of the reconstructed polygon.
    pub vertices: Vec<PlanarPoint>,
    /// Clipped polygon components referencing the shared vertices.
    pub polygons: Vec<IndexedPolygon>,
    /// Number of assigned cells before clipping their edge portions.
    pub cells: usize,
}

/// Partition the envelope by shortest terrain travel cost from located members.
pub fn derive(request: &TerrainRequest) -> Result<Vec<TerrainRegion>, TerrainError> {
    request.grid.validate()?;
    if request.units == 0 || !request.slope_weight.is_finite() || request.slope_weight < 0.0 {
        return Err(TerrainError::Input(
            "units and slope weight are invalid".into(),
        ));
    }
    let angular = resolve(&request.vertices, &request.envelope)?;
    if request
        .vertices
        .iter()
        .any(|p| !(-180.0..=180.0).contains(&p.x) || !(-85.0..=85.0).contains(&p.y))
    {
        return Err(TerrainError::Input(
            "envelope exceeds the Mercator domain".into(),
        ));
    }
    let envelope = angular.map_coords(|p| {
        project(LonLat {
            longitude: p.x,
            latitude: p.y,
        })
    });
    request.grid.encloses(&envelope)?;
    validate_seeds(request, &angular)?;
    let labels = fitting::assign(request, &envelope)?;
    let regions = vectorize::regions(request, &labels, &envelope)?;
    connectivity::validate_neighbors(request, &regions)?;
    Ok(regions)
}

fn validate_seeds(
    request: &TerrainRequest,
    envelope: &MultiPolygon<f64>,
) -> Result<(), TerrainError> {
    let mut present = std::collections::BTreeSet::new();
    for (index, seed) in request.seeds.iter().enumerate() {
        seed.point
            .validate()
            .map_err(|e| TerrainError::Input(e.to_string()))?;
        if seed.unit >= request.units
            || !envelope.contains(&Point::new(seed.point.longitude, seed.point.latitude))
        {
            return Err(TerrainError::Seed(index));
        }
        present.insert(seed.unit);
    }
    if present.len() != request.units {
        return Err(TerrainError::Input(
            "each unit needs a seed inside the envelope".into(),
        ));
    }
    if envelope.0.iter().any(|polygon| {
        !request
            .seeds
            .iter()
            .any(|seed| polygon.contains(&Point::new(seed.point.longitude, seed.point.latitude)))
    }) {
        return Err(TerrainError::Input(
            "each envelope component needs a seed".into(),
        ));
    }
    Ok(())
}

const RADIUS: f64 = 6_378_137.0;

fn project(point: LonLat) -> geo::Coord<f64> {
    geo::Coord {
        x: RADIUS * point.longitude.to_radians(),
        y: RADIUS * point.latitude.to_radians().tan().asinh(),
    }
}

fn unproject(point: geo::Coord<f64>) -> PlanarPoint {
    PlanarPoint {
        x: (point.x / RADIUS).to_degrees(),
        y: (point.y / RADIUS).sinh().atan().to_degrees(),
    }
}

/// Invalid terrain input, conflicting seeds, or reconstructed topology.
#[derive(Debug, Error)]
pub enum TerrainError {
    /// Grid dimensions, elevations, extent, or cost parameters are invalid.
    #[error("terrain input: {0}")]
    Input(String),
    /// A seed has an unknown unit or lies outside the enclosing polygon.
    #[error("terrain seed {0} is outside the partition or has an unknown unit")]
    Seed(usize),
    /// Different units claim the same elevation cell.
    #[error("terrain cell {cell} has conflicting seed units {first} and {second}")]
    Conflict {
        /// Row-major cell index.
        cell: usize,
        /// First unit claiming the cell.
        first: usize,
        /// Second unit claiming the cell.
        second: usize,
    },
    /// A cell cannot be addressed in this grid.
    #[error("terrain cell {0} is outside the grid")]
    Cell(usize),
    /// An enclosing component lacks a reachable seed.
    #[error("terrain envelope has {0} cells unreachable from its seeds")]
    Unreached(usize),
    /// Cell resolution or competing constraints prevent the requested area fit.
    #[error("terrain unit {unit} area {actual} km² cannot fit {target} ± {tolerance} km²")]
    AreaFit {
        /// Index in the ordered unit list.
        unit: usize,
        /// Last modeled area.
        actual: f64,
        /// Requested source area.
        target: f64,
        /// Permitted absolute area residual.
        tolerance: f64,
    },
    /// Source or reconstructed polygon failed topology validation.
    #[error("terrain polygon: {0}")]
    Topology(#[from] crate::polygon::TopologyError),
}

#[cfg(test)]
mod tests;
