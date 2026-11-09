//! Control-point registration in explicitly chosen planar source and target units.

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod affine;
mod fitting;
mod projective;
mod support;
mod thin_plate;

pub use affine::AffineRegistration;
pub use projective::ProjectiveRegistration;
pub use support::ControlSupport;
pub use thin_plate::ThinPlateSpline;

/// A point in a caller-declared planar coordinate reference system.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarPoint {
    /// Horizontal coordinate, such as image pixels or projected metres.
    pub x: f64,
    /// Vertical coordinate in the same declared frame.
    pub y: f64,
}

impl PlanarPoint {
    /// Reject nonfinite input before matrix fitting or evaluation.
    pub fn validate(self) -> Result<(), RegistrationError> {
        if self.x.is_finite() && self.y.is_finite() {
            Ok(())
        } else {
            Err(RegistrationError::NonfinitePoint(self))
        }
    }

    /// Euclidean distance in the shared planar units.
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// One independently identified correspondence between image and target frames.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlPoint {
    /// Location measured in the source image or map.
    pub source: PlanarPoint,
    /// Surveyed or otherwise established target location.
    pub target: PlanarPoint,
}

/// Error against supplied correspondences, in target coordinate units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationQuality {
    /// Number of evaluated correspondences.
    pub points: usize,
    /// Root mean square Euclidean error.
    pub rms: f64,
    /// Largest Euclidean error.
    pub maximum: f64,
    /// Per-point errors in input order.
    pub residuals: Vec<f64>,
}

/// Registration model capable of evaluating independent check points.
pub trait Registration {
    /// Transform a source point into target coordinates.
    fn transform(&self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError>;

    /// Measure error on supplied points, including holdouts excluded from fitting.
    fn evaluate(
        &self,
        controls: &[ControlPoint],
    ) -> Result<RegistrationQuality, RegistrationError> {
        if controls.is_empty() {
            return Err(RegistrationError::TooFewPoints {
                actual: 0,
                minimum: 1,
            });
        }
        let mut residuals = Vec::with_capacity(controls.len());
        for control in controls {
            control.target.validate()?;
            residuals.push(self.transform(control.source)?.distance(control.target));
        }
        let squared_sum: f64 = residuals.iter().map(|r| r * r).sum();
        let rms = (squared_sum / controls.len() as f64).sqrt();
        let maximum = residuals.iter().copied().fold(0.0, f64::max);
        if !rms.is_finite() || !maximum.is_finite() {
            return Err(RegistrationError::NumericalRange);
        }
        Ok(RegistrationQuality {
            points: controls.len(),
            rms,
            maximum,
            residuals,
        })
    }
}

/// Invalid controls, degenerate geometry, or failed numerical registration.
#[derive(Debug, Error)]
pub enum RegistrationError {
    /// Insufficient independent correspondences.
    #[error("registration requires at least {minimum} points, received {actual}")]
    TooFewPoints {
        /// Supplied point count.
        actual: usize,
        /// Required point count.
        minimum: usize,
    },
    /// Nonfinite coordinate input.
    #[error("registration point is nonfinite: {0:?}")]
    NonfinitePoint(PlanarPoint),
    /// Points do not establish a two-dimensional mapping.
    #[error("registration controls are duplicated, collinear, or numerically singular")]
    DegenerateControls,
    /// Invalid regularization parameter.
    #[error("thin-plate smoothing must be finite and nonnegative")]
    InvalidSmoothing,
    /// Numerical solver failed.
    #[error("registration linear solve: {0}")]
    Solve(&'static str),
    /// Point maps to or beyond the horizon of a projective registration.
    #[error("source point {0:?} lies on or beyond the registered ground plane's horizon")]
    BeyondHorizon(PlanarPoint),
    /// Point lies outside the region supported by the fitting controls.
    #[error("source point {0:?} lies outside the convex hull of the fitting controls")]
    OutsideSupport(PlanarPoint),
    /// Floating-point range exceeded during fitting or evaluation.
    #[error("registration computation exceeds finite numerical range")]
    NumericalRange,
}

#[cfg(test)]
mod tests;
