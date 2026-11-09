use atlas_geo::{camera::Terrain, coordinates::LonLat};

use super::{ElevationGrid, project};

/// Bilinear heights between cell centres, so camera rays meet a continuous
/// surface instead of stepping across cell edges.
impl Terrain for ElevationGrid {
    fn height_m(&self, position: LonLat) -> Option<f64> {
        let point = project(position);
        let column = (point.x - self.west_m) / self.cell_m - 0.5;
        let row = (self.north_m - point.y) / self.cell_m - 0.5;
        let last_column = self.width.checked_sub(1)? as f64;
        let last_row = self.height.checked_sub(1)? as f64;
        if !(0.0..=last_column).contains(&column) || !(0.0..=last_row).contains(&row) {
            return None;
        }
        let (left, top) = (column.floor(), row.floor());
        let (dx, dy) = (column - left, row - top);
        let sample = |x: f64, y: f64| -> Option<f64> {
            let index = y.min(last_row) as usize * self.width + x.min(last_column) as usize;
            self.elevations_m.get(index).map(|value| f64::from(*value))
        };
        let upper = sample(left, top)? * (1.0 - dx) + sample(left + 1.0, top)? * dx;
        let lower = sample(left, top + 1.0)? * (1.0 - dx) + sample(left + 1.0, top + 1.0)? * dx;
        Some(upper * (1.0 - dy) + lower * dy)
    }

    fn spacing_m(&self) -> f64 {
        // Web Mercator cells shrink on the ground by the cosine of latitude;
        // using the projected size keeps ray steps at or below half a cell.
        self.cell_m
    }
}

#[cfg(test)]
mod tests;
