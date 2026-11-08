use nalgebra::DMatrix;

use super::{
    ControlPoint, PlanarPoint, Registration, RegistrationError,
    fitting::{FittingPoints, Normalization, solve},
};

/// Thin-plate spline warp with an affine tail and optional smoothing.
#[derive(Debug, Clone)]
pub struct ThinPlateSpline {
    source_frame: Normalization,
    target_frame: Normalization,
    centers: Vec<PlanarPoint>,
    x: Vec<f64>,
    y: Vec<f64>,
}

impl ThinPlateSpline {
    /// Fit a warp; zero smoothing interpolates the controls exactly.
    ///
    /// Smoothing is measured in the internally normalized source frame.
    pub fn fit(controls: &[ControlPoint], smoothing: f64) -> Result<Self, RegistrationError> {
        if !smoothing.is_finite() || smoothing < 0.0 {
            return Err(RegistrationError::InvalidSmoothing);
        }
        let points = FittingPoints::new(controls)?;
        let count = points.source.len();
        let mut values = Vec::with_capacity((count + 3) * (count + 3));
        for (i, a) in points.source.iter().enumerate() {
            for (j, b) in points.source.iter().enumerate() {
                if i != j && a == b {
                    return Err(RegistrationError::DegenerateControls);
                }
                values.push(kernel(a.distance(*b)) + if i == j { smoothing } else { 0.0 });
            }
            values.extend([1.0, a.x, a.y]);
        }
        for component in 0..3 {
            values.extend(points.source.iter().map(|p| match component {
                0 => 1.0,
                1 => p.x,
                _ => p.y,
            }));
            values.extend([0.0; 3]);
        }
        let mut target = points.target;
        target.extend([PlanarPoint { x: 0.0, y: 0.0 }; 3]);
        let matrix = DMatrix::from_row_slice(count + 3, count + 3, &values);
        let (x, y) = solve(matrix, &target)?;
        Ok(Self {
            source_frame: points.source_frame,
            target_frame: points.target_frame,
            centers: points.source,
            x,
            y,
        })
    }
}

impl Registration for ThinPlateSpline {
    fn transform(&self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        let point = self.source_frame.normalize(point)?;
        let basis: Vec<_> = self
            .centers
            .iter()
            .map(|center| kernel(center.distance(point)))
            .chain([1.0, point.x, point.y])
            .collect();
        self.target_frame.restore(PlanarPoint {
            x: self.x.iter().zip(&basis).map(|(a, b)| a * b).sum(),
            y: self.y.iter().zip(&basis).map(|(a, b)| a * b).sum(),
        })
    }
}

fn kernel(radius: f64) -> f64 {
    if radius == 0.0 {
        0.0
    } else {
        radius * radius * radius.ln()
    }
}
