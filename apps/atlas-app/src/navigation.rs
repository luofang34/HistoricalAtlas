//! Mercator camera navigation in logical viewport pixels.

use atlas_geo::{body::BodyId, coordinates::LonLat};

/// Camera shared by pointer navigation and entity selection.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// Angular camera center.
    pub center: LonLat,
    /// MapLibre zoom level.
    pub zoom: f64,
}

impl Camera {
    /// Initial view for a body.
    pub fn for_body(body: BodyId) -> Self {
        match body {
            BodyId::Earth => Self {
                center: LonLat {
                    longitude: 105.27,
                    latitude: 27.3,
                },
                zoom: 10.0,
            },
            _ => Self {
                center: LonLat {
                    longitude: 0.0,
                    latitude: 0.0,
                },
                zoom: 1.0,
            },
        }
    }

    /// Move the map with a pointer drag, preserving longitude wrap.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let scale = 512.0 * 2.0_f64.powf(self.zoom);
        let [x, y] = project(self.center);
        self.center = unproject([x - dx / scale, y - dy / scale]);
    }

    /// Change zoom around a viewport offset measured from its center.
    pub fn zoom_at(&mut self, delta: f64, [x, y]: [f64; 2]) {
        self.pan(-x, -y);
        self.zoom = (self.zoom + delta).clamp(0.0, 18.0);
        self.pan(x, y);
    }

    /// Keep a flat viewport inside the projection's north and south edges.
    pub fn fit_height(&mut self, height: f64) {
        let height = height.max(1.0);
        self.zoom = self.zoom.max((height / 512.0).log2().max(0.0));
        let half = (height / (512.0 * 2.0_f64.powf(self.zoom)) / 2.0).min(0.5);
        let [x, y] = project(self.center);
        self.center = unproject([x, y.clamp(half, 1.0 - half)]);
    }
}

fn project(point: LonLat) -> [f64; 2] {
    let latitude = point
        .latitude
        .clamp(-85.0511287798066, 85.0511287798066)
        .to_radians();
    [
        (point.longitude + 180.0) / 360.0,
        (1.0 - latitude.tan().asinh() / std::f64::consts::PI) / 2.0,
    ]
}

fn unproject([x, y]: [f64; 2]) -> LonLat {
    LonLat {
        longitude: (x * 360.0).rem_euclid(360.0) - 180.0,
        latitude: ((1.0 - 2.0 * y.clamp(0.0, 1.0)) * std::f64::consts::PI)
            .sinh()
            .atan()
            .to_degrees(),
    }
}

#[cfg(test)]
mod tests;
