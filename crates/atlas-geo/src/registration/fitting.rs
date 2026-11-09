use nalgebra::{DMatrix, DVector, Matrix3};

use super::{ControlPoint, PlanarPoint, RegistrationError};

#[derive(Debug, Clone, Copy)]
pub(super) struct Normalization {
    origin: PlanarPoint,
    scale: f64,
}

impl Normalization {
    pub(super) fn fit(points: &[PlanarPoint]) -> Result<Self, RegistrationError> {
        let count = points.len() as f64;
        let origin = PlanarPoint {
            x: points.iter().map(|p| p.x / count).sum(),
            y: points.iter().map(|p| p.y / count).sum(),
        };
        origin.validate()?;
        let scale = points
            .iter()
            .map(|p| p.distance(origin))
            .fold(0.0, f64::max);
        if !scale.is_finite() || scale == 0.0 {
            return Err(RegistrationError::DegenerateControls);
        }
        Ok(Self { origin, scale })
    }

    pub(super) fn normalize(self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        point.validate()?;
        let result = PlanarPoint {
            x: (point.x - self.origin.x) / self.scale,
            y: (point.y - self.origin.y) / self.scale,
        };
        result.validate()?;
        Ok(result)
    }

    /// Homogeneous matrix applying [`Self::normalize`].
    pub(super) fn normalizing_matrix(self) -> Matrix3<f64> {
        let inverse = 1.0 / self.scale;
        Matrix3::new(
            inverse,
            0.0,
            -self.origin.x * inverse,
            0.0,
            inverse,
            -self.origin.y * inverse,
            0.0,
            0.0,
            1.0,
        )
    }

    /// Homogeneous matrix applying [`Self::restore`].
    pub(super) fn restoring_matrix(self) -> Matrix3<f64> {
        Matrix3::new(
            self.scale,
            0.0,
            self.origin.x,
            0.0,
            self.scale,
            self.origin.y,
            0.0,
            0.0,
            1.0,
        )
    }

    pub(super) fn restore(self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        let result = PlanarPoint {
            x: point.x * self.scale + self.origin.x,
            y: point.y * self.scale + self.origin.y,
        };
        result.validate()?;
        Ok(result)
    }
}

pub(super) struct FittingPoints {
    pub(super) source_frame: Normalization,
    pub(super) target_frame: Normalization,
    pub(super) source: Vec<PlanarPoint>,
    pub(super) target: Vec<PlanarPoint>,
}

impl FittingPoints {
    pub(super) fn new(controls: &[ControlPoint]) -> Result<Self, RegistrationError> {
        if controls.len() < 3 {
            return Err(RegistrationError::TooFewPoints {
                actual: controls.len(),
                minimum: 3,
            });
        }
        for control in controls {
            control.source.validate()?;
            control.target.validate()?;
        }
        let source: Vec<_> = controls.iter().map(|c| c.source).collect();
        let target: Vec<_> = controls.iter().map(|c| c.target).collect();
        let source_frame = Normalization::fit(&source)?;
        let target_frame = Normalization::fit(&target)?;
        let source = source
            .into_iter()
            .map(|p| source_frame.normalize(p))
            .collect::<Result<_, _>>()?;
        let target = target
            .into_iter()
            .map(|p| target_frame.normalize(p))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            source_frame,
            target_frame,
            source,
            target,
        })
    }
}

pub(super) fn solve(
    matrix: DMatrix<f64>,
    target: &[PlanarPoint],
) -> Result<(Vec<f64>, Vec<f64>), RegistrationError> {
    let columns = matrix.ncols();
    let rows = matrix.nrows();
    let svd = matrix.svd(true, true);
    let tolerance = svd.singular_values.iter().copied().fold(0.0, f64::max) * 1e-12;
    if svd
        .singular_values
        .iter()
        .filter(|value| **value > tolerance)
        .count()
        < columns
    {
        return Err(RegistrationError::DegenerateControls);
    }
    let x = DVector::from_iterator(rows, target.iter().map(|p| p.x));
    let y = DVector::from_iterator(rows, target.iter().map(|p| p.y));
    let x = svd.solve(&x, tolerance).map_err(RegistrationError::Solve)?;
    let y = svd.solve(&y, tolerance).map_err(RegistrationError::Solve)?;
    if !x.iter().chain(y.iter()).all(|value| value.is_finite()) {
        return Err(RegistrationError::NumericalRange);
    }
    Ok((x.iter().copied().collect(), y.iter().copied().collect()))
}
