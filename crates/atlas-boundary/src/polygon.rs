//! Indexed map polygons with explicit ring closure and validated planar topology.

use atlas_geo::registration::PlanarPoint;
use geo::orient::Direction;
use geo::{Area, Coord, LineString, MultiPolygon, Orient, Polygon, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod connectivity;
mod sampling;
pub use sampling::densify;

/// One polygon referencing a sheet's shared vertex array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedPolygon {
    /// Exterior ring, including the repeated first index at the end.
    pub exterior: Vec<usize>,
    /// Hole rings, each explicitly closed.
    pub interiors: Vec<Vec<usize>>,
}

/// Resolve shared vertices and validate a nonempty polygon or multipolygon.
pub fn resolve(
    vertices: &[PlanarPoint],
    polygons: &[IndexedPolygon],
) -> Result<MultiPolygon<f64>, TopologyError> {
    if polygons.is_empty() {
        return Err(TopologyError::Empty);
    }
    let mut resolved = Vec::new();
    for (index, polygon) in polygons.iter().enumerate() {
        let exterior = ring(vertices, &polygon.exterior)?;
        let interiors = polygon
            .interiors
            .iter()
            .map(|indices| ring(vertices, indices))
            .collect::<Result<_, _>>()?;
        let polygon = Polygon::new(exterior, interiors);
        polygon
            .check_validation()
            .map_err(|source| TopologyError::Polygon { index, source })?;
        connectivity::check(&polygon)?;
        let area = polygon.unsigned_area();
        if !area.is_finite() || area <= 0.0 {
            return Err(TopologyError::Area(index));
        }
        resolved.push(polygon);
    }
    let result = MultiPolygon(resolved);
    result.check_validation()?;
    Ok(result.orient(Direction::Default))
}

fn ring(vertices: &[PlanarPoint], indices: &[usize]) -> Result<LineString<f64>, TopologyError> {
    if indices.len() < 4 || indices.first() != indices.last() {
        return Err(TopologyError::Ring);
    }
    let points = indices
        .iter()
        .map(|index| {
            let point = vertices.get(*index).ok_or(TopologyError::Index(*index))?;
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(TopologyError::Coordinate(*index));
            }
            Ok(Coord {
                x: point.x,
                y: point.y,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if points.windows(2).any(|pair| pair.first() == pair.last()) {
        return Err(TopologyError::Repeated);
    }
    Ok(LineString(points))
}

/// Invalid trace indexing, coordinate, or polygon topology.
#[derive(Debug, Error)]
pub enum TopologyError {
    /// No polygon was supplied.
    #[error("boundary has no polygons")]
    Empty,
    /// A ring is short or unclosed.
    #[error("rings require at least four indices and explicit closure")]
    Ring,
    /// A ring references an absent vertex.
    #[error("missing boundary vertex {0}")]
    Index(usize),
    /// A vertex is not finite.
    #[error("boundary vertex {0} is non-finite")]
    Coordinate(usize),
    /// Consecutive coordinates coincide.
    #[error("boundary ring has a zero-length edge")]
    Repeated,
    /// Polygon area is zero or numerically unrepresentable.
    #[error("polygon {0} has zero or non-finite area")]
    Area(usize),
    /// A polygon violates a simple-feature invariant.
    #[error("polygon {index}: {source}")]
    Polygon {
        /// Position of the polygon in the boundary.
        index: usize,
        /// Underlying topology failure.
        #[source]
        source: geo::algorithm::validation::InvalidPolygon,
    },
    /// Rings disconnect the polygon interior.
    #[error("ring contacts disconnect the polygon interior")]
    Disconnected,
    /// Multipolygon components overlap or share an edge.
    #[error("boundary multipolygon: {0}")]
    MultiPolygon(#[from] geo::algorithm::validation::InvalidMultiPolygon),
    /// Source-edge sampling parameters exceed the supported budget.
    #[error("edge sampling requires a finite positive spacing and at most 1000000 vertices")]
    Sampling,
}

#[cfg(test)]
mod tests;
