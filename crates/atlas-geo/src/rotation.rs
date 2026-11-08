//! Finite Euler rotations applied to present-day body-fixed anchors.

use nalgebra::{Unit, UnitQuaternion, Vector3};
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
    /// Identity rotation, with a conventional north-pole axis.
    pub const IDENTITY: Self = Self {
        pole: LonLat {
            longitude: 0.0,
            latitude: 90.0,
        },
        angle_degrees: 0.0,
    };

    /// Apply this rotation followed by `next`.
    pub fn then(self, next: Self) -> Result<Self, RotationError> {
        Ok(Self::from_quaternion(
            next.quaternion()? * self.quaternion()?,
        ))
    }

    /// Reverse the rotation.
    pub fn inverse(self) -> Result<Self, RotationError> {
        self.quaternion()?;
        Ok(Self {
            angle_degrees: -self.angle_degrees,
            ..self
        })
    }

    /// Interpolate on the shortest quaternion arc, as in GPlates finite rotations.
    pub fn interpolate(self, other: Self, fraction: f64) -> Result<Self, RotationError> {
        if !(0.0..=1.0).contains(&fraction) {
            return Err(RotationError::Interpolation);
        }
        let first = self.quaternion()?;
        let last = other.quaternion()?;
        first
            .try_slerp(&last, fraction, 1e-12)
            .map(Self::from_quaternion)
            .ok_or(RotationError::Interpolation)
    }

    /// Angular difference between the resulting orientations, in degrees.
    pub fn separation_degrees(self, other: Self) -> Result<f64, RotationError> {
        Ok(self
            .quaternion()?
            .angle_to(&other.quaternion()?)
            .to_degrees())
    }

    fn quaternion(self) -> Result<UnitQuaternion<f64>, RotationError> {
        self.pole.validate()?;
        if !self.angle_degrees.is_finite() {
            return Err(RotationError::NonfiniteAngle);
        }
        let (x, y, z) = unit_vector(self.pole);
        let axis = Unit::new_normalize(Vector3::new(x, y, z));
        Ok(UnitQuaternion::from_axis_angle(
            &axis,
            self.angle_degrees.to_radians(),
        ))
    }

    fn from_quaternion(value: UnitQuaternion<f64>) -> Self {
        let Some((axis, angle)) = value.axis_angle() else {
            return Self::IDENTITY;
        };
        Self {
            pole: LonLat {
                longitude: axis.y.atan2(axis.x).to_degrees(),
                latitude: axis.z.atan2(axis.x.hypot(axis.y)).to_degrees(),
            },
            angle_degrees: angle.to_degrees(),
        }
    }

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
    /// Interpolation fraction or quaternion arc is invalid.
    #[error("rotation interpolation requires a fraction in [0, 1] and a defined arc")]
    Interpolation,
    /// Invalid pole or anchor.
    #[error("rotation coordinate: {0}")]
    Coordinate(#[from] CoordinateError),
    /// Nonfinite angle.
    #[error("Euler rotation angle must be finite")]
    NonfiniteAngle,
}

#[cfg(test)]
mod tests;
