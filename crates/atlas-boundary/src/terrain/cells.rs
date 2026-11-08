use super::{TerrainError, project};
use atlas_geo::coordinates::LonLat;
use geo::{BoundingRect, Coord, MultiPolygon, Rect};
use serde::{Deserialize, Serialize};

/// Regular Web Mercator elevation samples, stored north-to-south in row-major order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElevationGrid {
    /// Number of columns.
    pub width: usize,
    /// Number of rows.
    pub height: usize,
    /// Western cell edge in Web Mercator metres.
    pub west_m: f64,
    /// Northern cell edge in Web Mercator metres.
    pub north_m: f64,
    /// Square cell size in projected metres.
    pub cell_m: f64,
    /// Cell-center elevations in metres; void samples are rejected.
    pub elevations_m: Vec<i16>,
}

impl ElevationGrid {
    /// Validate dimensions, sample count, finite georeferencing, and voids.
    pub fn validate(&self) -> Result<(), TerrainError> {
        let count = self.width.checked_mul(self.height);
        if self.width == 0
            || self.height == 0
            || count != Some(self.elevations_m.len())
            || self.elevations_m.len() > 1_000_000
            || !self.west_m.is_finite()
            || !self.north_m.is_finite()
            || !self.cell_m.is_finite()
            || self.cell_m <= 0.0
            || self.elevations_m.contains(&i16::MIN)
        {
            return Err(TerrainError::Input(
                "invalid elevation grid or void sample".into(),
            ));
        }
        Ok(())
    }

    /// Resolve a location to its row-major cell, excluding the east and south edges.
    pub fn cell(&self, point: LonLat) -> Option<usize> {
        let point = project(point);
        let x = ((point.x - self.west_m) / self.cell_m).floor();
        let y = ((self.north_m - point.y) / self.cell_m).floor();
        if x >= 0.0 && x < self.width as f64 && y >= 0.0 && y < self.height as f64 {
            Some(y as usize * self.width + x as usize)
        } else {
            None
        }
    }

    pub(super) fn rectangle(&self, row: usize, start: usize, end: usize) -> Rect<f64> {
        Rect::new(
            Coord {
                x: self.west_m + start as f64 * self.cell_m,
                y: self.north_m - (row + 1) as f64 * self.cell_m,
            },
            Coord {
                x: self.west_m + end as f64 * self.cell_m,
                y: self.north_m - row as f64 * self.cell_m,
            },
        )
    }

    pub(super) fn encloses(&self, polygon: &MultiPolygon<f64>) -> Result<(), TerrainError> {
        let bounds = polygon
            .bounding_rect()
            .ok_or_else(|| TerrainError::Input("empty envelope".into()))?;
        if bounds.min().x < self.west_m
            || bounds.max().x > self.west_m + self.width as f64 * self.cell_m
            || bounds.max().y > self.north_m
            || bounds.min().y < self.north_m - self.height as f64 * self.cell_m
        {
            return Err(TerrainError::Input(
                "elevation grid does not enclose the polygon".into(),
            ));
        }
        Ok(())
    }
}
