use nalgebra::{DMatrix, Matrix3};

use super::{
    ControlPoint, PlanarPoint, Registration, RegistrationError,
    fitting::{FittingPoints, Normalization},
};

/// Smallest normalized homogeneous scale accepted as being in front of the camera.
///
/// Normalized control coordinates lie within the unit disc, so the controls
/// themselves sit far above this bound; points near the image of the horizon
/// approach zero and are rejected rather than mapped to unbounded coordinates.
const MINIMUM_HOMOGENEOUS_SCALE: f64 = 1e-6;

/// Planar projective transform (homography) fitted by the normalized direct linear transform.
///
/// A pinhole photograph of flat ground relates image pixels to ground
/// coordinates by exactly this model, so it registers oblique photographs of
/// level terrain without lens or camera-height assumptions. It does not model
/// relief, lens distortion, or anything above the ground plane.
#[derive(Debug, Clone)]
pub struct ProjectiveRegistration {
    source_frame: Normalization,
    target_frame: Normalization,
    matrix: [f64; 9],
    orientation: f64,
}

impl ProjectiveRegistration {
    /// Fit at least four correspondences, no three of which are collinear.
    pub fn fit(controls: &[ControlPoint]) -> Result<Self, RegistrationError> {
        if controls.len() < 4 {
            return Err(RegistrationError::TooFewPoints {
                actual: controls.len(),
                minimum: 4,
            });
        }
        let points = FittingPoints::new(controls)?;
        let mut values = Vec::with_capacity(18 * points.source.len().max(5));
        for (source, target) in points.source.iter().zip(&points.target) {
            let (x, y, u, v) = (source.x, source.y, target.x, target.y);
            values.extend([x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, -u]);
            values.extend([0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, -v]);
        }
        // Zero rows keep the decomposition square so the null vector is always returned.
        let rows = (values.len() / 9).max(9);
        values.resize(rows * 9, 0.0);
        let design = DMatrix::from_row_slice(rows, 9, &values);
        let matrix = null_vector(design)?;
        let mut model = Self {
            source_frame: points.source_frame,
            target_frame: points.target_frame,
            matrix,
            orientation: 1.0,
        };
        let scales: Vec<_> = points.source.iter().map(|p| model.scale(*p)).collect();
        model.orientation = scales.first().copied().unwrap_or(1.0).signum();
        if scales
            .iter()
            .any(|scale| scale * model.orientation <= MINIMUM_HOMOGENEOUS_SCALE)
        {
            return Err(RegistrationError::DegenerateControls);
        }
        Ok(model)
    }

    /// Homography in the caller's source and target units, normalized to unit Frobenius norm.
    pub fn matrix(&self) -> Matrix3<f64> {
        let normalized = Matrix3::from_row_slice(&self.matrix);
        let matrix = self.target_frame.restoring_matrix()
            * normalized
            * self.source_frame.normalizing_matrix();
        matrix / matrix.norm()
    }

    fn scale(&self, point: PlanarPoint) -> f64 {
        let [_, _, _, _, _, _, g, h, i] = self.matrix;
        g * point.x + h * point.y + i
    }
}

impl Registration for ProjectiveRegistration {
    fn transform(&self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        let normalized = self.source_frame.normalize(point)?;
        let scale = self.scale(normalized) * self.orientation;
        if scale <= MINIMUM_HOMOGENEOUS_SCALE {
            return Err(RegistrationError::BeyondHorizon(point));
        }
        let [a, b, c, d, e, f, ..] = self.matrix;
        let w = scale * self.orientation;
        self.target_frame.restore(PlanarPoint {
            x: (a * normalized.x + b * normalized.y + c) / w,
            y: (d * normalized.x + e * normalized.y + f) / w,
        })
    }
}

/// Unit vector spanning the design matrix's one-dimensional null space.
fn null_vector(design: DMatrix<f64>) -> Result<[f64; 9], RegistrationError> {
    let svd = design.svd(false, true);
    let v_t = svd.v_t.ok_or(RegistrationError::Solve("homography SVD"))?;
    let mut ranked: Vec<_> = svd
        .singular_values
        .iter()
        .copied()
        .zip(v_t.row_iter())
        .collect();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let [(_, row), (next, _), ..] = ranked.as_slice() else {
        return Err(RegistrationError::Solve("homography dimensions"));
    };
    let largest = svd.singular_values.iter().copied().fold(0.0, f64::max);
    // A second near-zero singular value means the controls admit a family of
    // homographies: three or more are collinear or coincident.
    if *next <= largest * 1e-9 {
        return Err(RegistrationError::DegenerateControls);
    }
    let values: Vec<f64> = row.iter().copied().collect();
    let matrix: [f64; 9] = values
        .try_into()
        .map_err(|_| RegistrationError::Solve("homography dimensions"))?;
    if !matrix.iter().all(|value| value.is_finite()) {
        return Err(RegistrationError::NumericalRange);
    }
    let [a, b, c, d, e, f, g, h, i] = matrix;
    let determinant = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if determinant.abs() <= 1e-12 {
        return Err(RegistrationError::DegenerateControls);
    }
    Ok(matrix)
}
