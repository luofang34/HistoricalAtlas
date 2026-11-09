use super::{ElevationGrid, TerrainError, TerrainRegion, TerrainRequest, unproject};
use crate::polygon::{IndexedPolygon, resolve};
use atlas_geo::registration::PlanarPoint;
use geo::{BooleanOps, LineString, MultiPolygon, Polygon, unary_union};

pub(super) fn regions(
    request: &TerrainRequest,
    labels: &[Option<usize>],
    envelope: &MultiPolygon<f64>,
) -> Result<Vec<TerrainRegion>, TerrainError> {
    (0..request.units)
        .map(|unit| region(request, labels, envelope, unit))
        .collect()
}

pub(super) fn region(
    request: &TerrainRequest,
    labels: &[Option<usize>],
    envelope: &MultiPolygon<f64>,
    unit: usize,
) -> Result<TerrainRegion, TerrainError> {
    let mut pieces = Vec::new();
    let mut cells = 0_usize;
    for (row, owners) in labels.chunks(request.grid.width).enumerate() {
        let mut start = None;
        for (col, owner) in owners.iter().chain(std::iter::once(&None)).enumerate() {
            if *owner == Some(unit) {
                start.get_or_insert(col);
                cells = cells.wrapping_add(1);
            } else if let Some(first) = start.take() {
                pieces.push(request.grid.rectangle(row, first, col).to_polygon());
            }
        }
    }
    let geometry = unary_union(&pieces).intersection(envelope);
    index_region(unit, cells, geometry, &request.grid)
}

fn index_region(
    unit: usize,
    cells: usize,
    geometry: MultiPolygon<f64>,
    grid: &ElevationGrid,
) -> Result<TerrainRegion, TerrainError> {
    let mut vertices = Vec::new();
    let polygons = geometry
        .0
        .iter()
        .map(|polygon: &Polygon<f64>| IndexedPolygon {
            exterior: index_ring(polygon.exterior(), &mut vertices, grid),
            interiors: polygon
                .interiors()
                .iter()
                .map(|r| index_ring(r, &mut vertices, grid))
                .collect(),
        })
        .collect::<Vec<_>>();
    resolve(&vertices, &polygons)?;
    Ok(TerrainRegion {
        unit,
        vertices,
        polygons,
        cells,
    })
}

fn index_ring(
    ring: &LineString<f64>,
    vertices: &mut Vec<PlanarPoint>,
    grid: &ElevationGrid,
) -> Vec<usize> {
    let start = vertices.len();
    vertices.extend(
        ring.0
            .iter()
            .take(ring.0.len().saturating_sub(1))
            .map(|p| vertex(*p, grid)),
    );
    let mut indices: Vec<_> = (start..vertices.len()).collect();
    indices.push(start);
    indices
}

fn vertex(mut point: geo::Coord<f64>, grid: &ElevationGrid) -> PlanarPoint {
    let x = grid.west_m + ((point.x - grid.west_m) / grid.cell_m).round() * grid.cell_m;
    let y = grid.north_m - ((grid.north_m - point.y) / grid.cell_m).round() * grid.cell_m;
    // Boolean clipping quantizes against the whole envelope, not an individual cell.
    let tolerance = grid.cell_m * grid.width.max(grid.height) as f64 * 1e-8;
    if (point.x - x).abs() <= tolerance {
        point.x = x;
    }
    if (point.y - y).abs() <= tolerance {
        point.y = y;
    }
    unproject(point)
}
