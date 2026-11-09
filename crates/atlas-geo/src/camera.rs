//! Pinhole cameras posed over terrain, for photographs whose relief or obliquity a planar fit cannot represent.
//!
//! Image pixels have integer centres, columns increase to the right and rows
//! downward. Camera axes are `+x` right, `+y` down and `+z` along the optical
//! axis. World coordinates are metres in a [`LocalFrame`](crate::geodesy::LocalFrame)
//! with axes east, north and up. Lens distortion is not modelled; scanned prints
//! of unknown cameras rarely support calibrating it.

use nalgebra::{Rotation3, Vector3};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::registration::{PlanarPoint, RegistrationError};

mod intersection;
mod resection;

pub use intersection::{Terrain, intersect};
pub use resection::{CameraControl, CameraFit, ResectionOptions, resect};

/// Undistorted pinhole intrinsics in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinholeCamera {
    /// Focal length in pixels, equal on both axes.
    pub focal_px: f64,
    /// Principal point in image pixels.
    pub principal: PlanarPoint,
}

/// Camera position and orientation in a local east-north-up frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose {
    /// Optical centre in local metres.
    pub centre: Vector3<f64>,
    /// Rotation taking world vectors into camera axes.
    pub world_to_camera: Rotation3<f64>,
}

/// Orientation summary for review: compass heading of the optical axis,
/// its depression below the horizon, and roll about it, all in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewAngles {
    /// Clockwise from grid north in the local frame.
    pub heading_deg: f64,
    /// Positive when looking below the horizon; 90 is a vertical photograph.
    pub depression_deg: f64,
    /// Rotation of the image's right axis above the horizon.
    pub roll_deg: f64,
}

impl CameraPose {
    /// Project a local point to pixels; points behind the camera have no image.
    pub fn project(&self, camera: &PinholeCamera, world: Vector3<f64>) -> Option<PlanarPoint> {
        let eye = self.world_to_camera * (world - self.centre);
        (eye.z > 1e-6).then(|| PlanarPoint {
            x: camera.principal.x + camera.focal_px * eye.x / eye.z,
            y: camera.principal.y + camera.focal_px * eye.y / eye.z,
        })
    }

    /// Unit world direction of the ray through a pixel.
    pub fn ray(&self, camera: &PinholeCamera, pixel: PlanarPoint) -> Vector3<f64> {
        let eye = Vector3::new(
            (pixel.x - camera.principal.x) / camera.focal_px,
            (pixel.y - camera.principal.y) / camera.focal_px,
            1.0,
        );
        (self.world_to_camera.inverse() * eye).normalize()
    }

    /// Heading, depression and roll of the optical axis.
    pub fn view_angles(&self) -> ViewAngles {
        let to_world = self.world_to_camera.inverse();
        let forward = to_world * Vector3::z();
        let right = to_world * Vector3::x();
        ViewAngles {
            heading_deg: forward.x.atan2(forward.y).to_degrees().rem_euclid(360.0),
            depression_deg: (-forward.z).asin().to_degrees(),
            roll_deg: right.z.asin().to_degrees(),
        }
    }
}

/// Camera fitting or terrain intersection failure.
#[derive(Debug, Error)]
pub enum CameraError {
    /// Planar initialization failed.
    #[error("camera initialization: {0}")]
    Registration(#[from] RegistrationError),
    /// The focal length cannot be recovered from the controls and no value was supplied.
    #[error(
        "focal length is not determined by these controls; supply a focal length for near-vertical views"
    )]
    FocalUnresolved,
    /// The solver did not reach a pose with every control in front of the camera.
    #[error("camera pose did not converge: {0}")]
    NoConvergence(&'static str),
    /// Invalid intrinsics or options.
    #[error("invalid camera parameter: {0}")]
    Invalid(&'static str),
    /// A pixel's ray does not reach the terrain within the model.
    #[error("ray through {0:?} leaves the terrain model or the sky before reaching ground")]
    NoGround(PlanarPoint),
}

#[cfg(test)]
mod tests;
