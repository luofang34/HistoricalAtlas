//! Finite Euler rotations applied to present-day body-fixed anchors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::coordinates::{CoordinateError, LonLat};

/// A finite right-handed Euler rotation about an axis through the body centre.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EulerRotation {
    /// Euler pole in body-fixed degrees.
    pub pole: LonLat,
    /// Right-handed rotation angle in degrees.
    pub angle_degrees: f64,
}

impl EulerRotation {
    /// Reconstruct a location without modifying the stored observation.
    pub fn apply(self, anchor: LonLat) -> Result<LonLat, RotationError> {
        self.pole.validate()?;
        anchor.validate()?;
        if !self.angle_degrees.is_finite() {
            return Err(RotationError::NonfiniteAngle);
        }
        let (px, py, pz) = unit_vector(self.pole);
        let (x, y, z) = unit_vector(anchor);
        let (sin, cos) = self.angle_degrees.to_radians().sin_cos();
        let dot = px * x + py * y + pz * z;
        let rx = x * cos + (py * z - pz * y) * sin + px * dot * (1.0 - cos);
        let ry = y * cos + (pz * x - px * z) * sin + py * dot * (1.0 - cos);
        let rz = z * cos + (px * y - py * x) * sin + pz * dot * (1.0 - cos);
        Ok(LonLat {
            longitude: ry.atan2(rx).to_degrees(),
            latitude: rz.atan2(rx.hypot(ry)).to_degrees(),
        })
    }
}

fn unit_vector(point: LonLat) -> (f64, f64, f64) {
    let (lat_sin, lat_cos) = point.latitude.to_radians().sin_cos();
    let (lon_sin, lon_cos) = point.longitude.to_radians().sin_cos();
    (lat_cos * lon_cos, lat_cos * lon_sin, lat_sin)
}

/// Failed finite rotation.
#[derive(Debug, Error)]
pub enum RotationError {
    /// Invalid pole or anchor.
    #[error("rotation coordinate: {0}")]
    Coordinate(#[from] CoordinateError),
    /// Nonfinite angle.
    #[error("Euler rotation angle must be finite")]
    NonfiniteAngle,
}

#[cfg(test)]
mod tests;
