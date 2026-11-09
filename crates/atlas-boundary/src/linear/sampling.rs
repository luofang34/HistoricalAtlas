use super::{LineError, TracedLine, validate};
use atlas_geo::registration::PlanarPoint;

/// Subdivide open paths in source pixels before applying nonlinear registration.
pub fn densify(
    vertices: &[PlanarPoint],
    lines: &[TracedLine],
    spacing: f64,
) -> Result<(Vec<PlanarPoint>, Vec<TracedLine>), LineError> {
    if !spacing.is_finite() || spacing <= 0.0 || vertices.len() > 1_000_000 {
        return Err(LineError::Sampling);
    }
    let mut points = vertices.to_vec();
    let mut result = Vec::new();
    for line in lines {
        validate(vertices, line)?;
        let mut line = line.clone();
        for path in &mut line.paths {
            let mut sampled = path.first().copied().into_iter().collect::<Vec<_>>();
            for pair in path.windows(2) {
                let [a, b] = pair else {
                    return Err(LineError::Path);
                };
                let from = *points.get(*a).ok_or(LineError::Index(*a))?;
                let to = *points.get(*b).ok_or(LineError::Index(*b))?;
                let count = ((to.x - from.x).hypot(to.y - from.y) / spacing)
                    .ceil()
                    .max(1.0);
                if !count.is_finite() || count + points.len() as f64 > 1_000_000.0 {
                    return Err(LineError::Sampling);
                }
                for step in 1..count as usize {
                    let fraction = step as f64 / count;
                    sampled.push(points.len());
                    points.push(PlanarPoint {
                        x: from.x + fraction * (to.x - from.x),
                        y: from.y + fraction * (to.y - from.y),
                    });
                }
                sampled.push(*b);
            }
            *path = sampled;
        }
        result.push(line);
    }
    Ok((points, result))
}
