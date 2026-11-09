//! WGS84 ellipsoidal heights and a local east-north-up frame for metric camera geometry.

use nalgebra::{Matrix3, Vector3};

use crate::coordinates::LonLat;

const SEMI_MAJOR_M: f64 = 6_378_137.0;
const FLATTENING: f64 = 1.0 / 298.257_223_563;

/// A geographic position with height above the reference surface in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeodeticPoint {
    /// Horizontal position in degrees.
    pub position: LonLat,
    /// Height in metres. Terrain models supply orthometric heights; the local
    /// frame treats them as ellipsoidal, which is a constant offset of tens of
    /// metres that cancels when controls and terrain share the same model.
    pub height_m: f64,
}

/// Tangent-plane frame with axes east, north and up at an origin on WGS84.
#[derive(Debug, Clone, Copy)]
pub struct LocalFrame {
    origin_ecef: Vector3<f64>,
    /// Rows are the east, north and up unit vectors in ECEF.
    ecef_to_local: Matrix3<f64>,
}

impl LocalFrame {
    /// Frame whose origin is the given geodetic point.
    pub fn new(origin: GeodeticPoint) -> Self {
        let (longitude, latitude) = (
            origin.position.longitude.to_radians(),
            origin.position.latitude.to_radians(),
        );
        let (sin_lon, cos_lon) = longitude.sin_cos();
        let (sin_lat, cos_lat) = latitude.sin_cos();
        let ecef_to_local = Matrix3::new(
            -sin_lon,
            cos_lon,
            0.0,
            -sin_lat * cos_lon,
            -sin_lat * sin_lon,
            cos_lat,
            cos_lat * cos_lon,
            cos_lat * sin_lon,
            sin_lat,
        );
        Self {
            origin_ecef: to_ecef(origin),
            ecef_to_local,
        }
    }

    /// East, north and up metres of a geodetic point relative to the origin.
    pub fn to_local(&self, point: GeodeticPoint) -> Vector3<f64> {
        self.ecef_to_local * (to_ecef(point) - self.origin_ecef)
    }

    /// Geodetic position of a local east-north-up vector.
    pub fn to_geodetic(&self, local: Vector3<f64>) -> GeodeticPoint {
        from_ecef(self.origin_ecef + self.ecef_to_local.transpose() * local)
    }
}

fn eccentricity_squared() -> f64 {
    FLATTENING * (2.0 - FLATTENING)
}

fn to_ecef(point: GeodeticPoint) -> Vector3<f64> {
    let (longitude, latitude) = (
        point.position.longitude.to_radians(),
        point.position.latitude.to_radians(),
    );
    let e2 = eccentricity_squared();
    let radius = SEMI_MAJOR_M / (1.0 - e2 * latitude.sin().powi(2)).sqrt();
    let horizontal = (radius + point.height_m) * latitude.cos();
    Vector3::new(
        horizontal * longitude.cos(),
        horizontal * longitude.sin(),
        (radius * (1.0 - e2) + point.height_m) * latitude.sin(),
    )
}

/// Bowring-style fixed-point inversion; five iterations reach sub-millimetre height.
fn from_ecef(ecef: Vector3<f64>) -> GeodeticPoint {
    let e2 = eccentricity_squared();
    let longitude = ecef.y.atan2(ecef.x);
    let horizontal = ecef.x.hypot(ecef.y);
    let mut latitude = ecef.z.atan2(horizontal * (1.0 - e2));
    let mut height = 0.0;
    for _ in 0..5 {
        let radius = SEMI_MAJOR_M / (1.0 - e2 * latitude.sin().powi(2)).sqrt();
        height = horizontal / latitude.cos() - radius;
        latitude = ecef
            .z
            .atan2(horizontal * (1.0 - e2 * radius / (radius + height)));
    }
    GeodeticPoint {
        position: LonLat {
            longitude: longitude.to_degrees(),
            latitude: latitude.to_degrees(),
        },
        height_m: height,
    }
}

#[cfg(test)]
mod tests;
