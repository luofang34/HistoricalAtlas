use nalgebra::DMatrix;

use super::{
    ControlPoint, PlanarPoint, Registration, RegistrationError,
    fitting::{FittingPoints, Normalization, solve},
};

/// Least-squares affine registration with centred, scaled control coordinates.
#[derive(Debug, Clone)]
pub struct AffineRegistration {
    source_frame: Normalization,
    target_frame: Normalization,
    x: [f64; 3],
    y: [f64; 3],
}

impl AffineRegistration {
    /// Fit at least three noncollinear correspondences in caller-declared units.
    pub fn fit(controls: &[ControlPoint]) -> Result<Self, RegistrationError> {
        let points = FittingPoints::new(controls)?;
        let values: Vec<_> = points.source.iter().flat_map(|p| [1.0, p.x, p.y]).collect();
        let design = DMatrix::from_row_slice(controls.len(), 3, &values);
        let (x, y) = solve(design, &points.target)?;
        let x: [f64; 3] = x
            .try_into()
            .map_err(|_| RegistrationError::Solve("affine x dimensions"))?;
        let y: [f64; 3] = y
            .try_into()
            .map_err(|_| RegistrationError::Solve("affine y dimensions"))?;
        let [_, xx, xy] = x;
        let [_, yx, yy] = y;
        let determinant = xx * yy - xy * yx;
        if determinant.abs() <= 1e-12 * xx.hypot(xy) * yx.hypot(yy) {
            return Err(RegistrationError::DegenerateControls);
        }
        Ok(Self {
            source_frame: points.source_frame,
            target_frame: points.target_frame,
            x,
            y,
        })
    }
}

impl Registration for AffineRegistration {
    fn transform(&self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        let point = self.source_frame.normalize(point)?;
        let [x0, xx, xy] = self.x;
        let [y0, yx, yy] = self.y;
        self.target_frame.restore(PlanarPoint {
            x: x0 + xx * point.x + xy * point.y,
            y: y0 + yx * point.x + yy * point.y,
        })
    }
}
