//! Named globe views and regional framing in logical viewport pixels.

use atlas_geo::{body::BodyId, coordinates::LonLat};

/// Spherical projection at both world and settlement scales.
pub const PROJECTION: maplibre::projection::ProjectionType =
    maplibre::projection::ProjectionType::VerticalPerspective;

/// A requested camera view for body or entity selection.
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
                    longitude: 0.0,
                    latitude: 0.0,
                },
                zoom: 0.5,
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

    /// Frame a non-wrapping geographic extent with space around its edges.
    pub fn fit_bounds(&mut self, [low, high]: [LonLat; 2], viewport: [f64; 2]) {
        self.center = LonLat {
            longitude: (low.longitude + high.longitude) / 2.0,
            latitude: (low.latitude + high.latitude) / 2.0,
        };
        let center = project(self.center);
        let a = project(low);
        let b = project(high);
        self.zoom = a
            .into_iter()
            .zip(b)
            .zip(center)
            .zip(viewport)
            .map(|(((a, b), center), pixels)| {
                let span = 2.0 * (a - center).abs().max((b - center).abs());
                (pixels.max(1.0) * 0.85 / (512.0 * span.max(1e-12))).log2()
            })
            .fold(18.0_f64, f64::min)
            .clamp(0.0, 18.0);
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

#[cfg(test)]
mod tests;
