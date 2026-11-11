//! Ellipsoidal heights and a local east-north-up frame for metric camera geometry on any body.

use nalgebra::{Matrix3, Vector3};

use crate::{
    body::{BodyEllipsoid, BodyId},
    coordinates::LonLat,
};

/// A geographic position with height above the reference surface in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeodeticPoint {
    /// Horizontal position in degrees.
    pub position: LonLat,
    /// Height in metres. Terrain models supply heights above their own datum
    /// surface; the local frame treats them as ellipsoidal, which is a nearly
    /// constant offset that cancels when controls and terrain share one model.
    pub height_m: f64,
}

/// Tangent-plane frame with axes east, north and up at an origin on a body's ellipsoid.
#[derive(Debug, Clone, Copy)]
pub struct LocalFrame {
    ellipsoid: BodyEllipsoid,
    origin_ecef: Vector3<f64>,
    /// Rows are the east, north and up unit vectors in body-fixed Cartesian axes.
    ecef_to_local: Matrix3<f64>,
}

impl LocalFrame {
    /// Frame on the Earth's WGS84 ellipsoid.
    pub fn new(origin: GeodeticPoint) -> Self {
        Self::with_ellipsoid(BodyId::Earth.ellipsoid(), origin)
    }

    /// Frame on a given reference ellipsoid, such as `datum.body()?.ellipsoid()`.
    pub fn with_ellipsoid(ellipsoid: BodyEllipsoid, origin: GeodeticPoint) -> Self {
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
            ellipsoid,
            origin_ecef: to_ecef(ellipsoid, origin),
            ecef_to_local,
        }
    }

    /// East, north and up metres of a geodetic point relative to the origin.
    pub fn to_local(&self, point: GeodeticPoint) -> Vector3<f64> {
        self.ecef_to_local * (to_ecef(self.ellipsoid, point) - self.origin_ecef)
    }

    /// Geodetic position of a local east-north-up vector.
    pub fn to_geodetic(&self, local: Vector3<f64>) -> GeodeticPoint {
        from_ecef(
            self.ellipsoid,
            self.origin_ecef + self.ecef_to_local.transpose() * local,
        )
    }
}

fn eccentricity_squared(ellipsoid: BodyEllipsoid) -> f64 {
    ellipsoid.flattening * (2.0 - ellipsoid.flattening)
}

fn to_ecef(ellipsoid: BodyEllipsoid, point: GeodeticPoint) -> Vector3<f64> {
    let (longitude, latitude) = (
        point.position.longitude.to_radians(),
        point.position.latitude.to_radians(),
    );
    let e2 = eccentricity_squared(ellipsoid);
    let radius = ellipsoid.semi_major_m / (1.0 - e2 * latitude.sin().powi(2)).sqrt();
    let horizontal = (radius + point.height_m) * latitude.cos();
    Vector3::new(
        horizontal * longitude.cos(),
        horizontal * longitude.sin(),
        (radius * (1.0 - e2) + point.height_m) * latitude.sin(),
    )
}

/// Bowring-style fixed-point inversion; five iterations reach sub-millimetre height.
fn from_ecef(ellipsoid: BodyEllipsoid, ecef: Vector3<f64>) -> GeodeticPoint {
    let e2 = eccentricity_squared(ellipsoid);
    let horizontal = ecef.x.hypot(ecef.y);
    if horizontal <= 1e-9 {
        let latitude = if ecef.z >= 0.0 { 90.0 } else { -90.0 };
        let polar_radius = ellipsoid.semi_major_m * (1.0 - ellipsoid.flattening);
        return GeodeticPoint {
            position: LonLat {
                longitude: 0.0,
                latitude,
            },
            height_m: ecef.z.abs() - polar_radius,
        };
    }
    let longitude = ecef.y.atan2(ecef.x);
    let mut latitude = ecef.z.atan2(horizontal * (1.0 - e2));
    let mut height = 0.0;
    for _ in 0..5 {
        let sin_lat = latitude.sin();
        let radius = ellipsoid.semi_major_m / (1.0 - e2 * sin_lat.powi(2)).sqrt();
        height = if latitude.abs() > 89.0_f64.to_radians() {
            ecef.z / sin_lat - radius * (1.0 - e2)
        } else {
            horizontal / latitude.cos() - radius
        };
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
