use super::{ControlPoint, PlanarPoint, RegistrationError};

/// Convex hull of the fitting controls in the source frame.
///
/// Oblique photographs and other projective fits are only constrained between
/// their controls; outside the hull, small control errors grow without bound
/// toward the horizon. Callers that must not extrapolate test points here.
#[derive(Debug, Clone)]
pub struct ControlSupport {
    vertices: Vec<PlanarPoint>,
}

impl ControlSupport {
    /// Hull of at least three non-collinear control source points.
    pub fn new(controls: &[ControlPoint]) -> Result<Self, RegistrationError> {
        let mut points: Vec<_> = controls.iter().map(|control| control.source).collect();
        for point in &points {
            point.validate()?;
        }
        points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        points.dedup();
        if points.len() < 3 {
            return Err(RegistrationError::TooFewPoints {
                actual: points.len(),
                minimum: 3,
            });
        }
        let mut lower: Vec<PlanarPoint> = Vec::with_capacity(points.len());
        for point in &points {
            push_turning(&mut lower, *point);
        }
        let mut upper: Vec<PlanarPoint> = Vec::with_capacity(points.len());
        for point in points.iter().rev() {
            push_turning(&mut upper, *point);
        }
        lower.pop();
        upper.pop();
        lower.extend(upper);
        if lower.len() < 3 {
            return Err(RegistrationError::DegenerateControls);
        }
        Ok(Self { vertices: lower })
    }

    /// Hull vertices in counter-clockwise order for a y-up frame.
    pub fn vertices(&self) -> &[PlanarPoint] {
        &self.vertices
    }

    /// Whether a point lies inside or on the hull boundary.
    pub fn contains(&self, point: PlanarPoint) -> bool {
        let scale = self
            .vertices
            .iter()
            .map(|vertex| vertex.x.abs().max(vertex.y.abs()))
            .fold(1.0, f64::max);
        let tolerance = scale * scale * 1e-12;
        self.vertices
            .iter()
            .zip(self.vertices.iter().cycle().skip(1))
            .all(|(a, b)| cross(*a, *b, point) >= -tolerance)
    }

    /// Reject a point outside the hull.
    pub fn require(&self, point: PlanarPoint) -> Result<(), RegistrationError> {
        point.validate()?;
        if self.contains(point) {
            Ok(())
        } else {
            Err(RegistrationError::OutsideSupport(point))
        }
    }
}

fn push_turning(chain: &mut Vec<PlanarPoint>, point: PlanarPoint) {
    while let [.., a, b] = chain.as_slice() {
        if cross(*a, *b, point) > 0.0 {
            break;
        }
        chain.pop();
    }
    chain.push(point);
}

fn cross(origin: PlanarPoint, a: PlanarPoint, b: PlanarPoint) -> f64 {
    (a.x - origin.x) * (b.y - origin.y) - (a.y - origin.y) * (b.x - origin.x)
}
