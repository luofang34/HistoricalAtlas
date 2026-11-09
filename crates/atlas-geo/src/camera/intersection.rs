use nalgebra::Vector3;

use super::{CameraError, CameraPose, PinholeCamera};
use crate::{
    coordinates::LonLat,
    geodesy::{GeodeticPoint, LocalFrame},
    registration::PlanarPoint,
};

/// Ground heights in metres, sampled where the model has data.
pub trait Terrain {
    /// Height at a position, or `None` outside the model.
    fn height_m(&self, position: LonLat) -> Option<f64>;

    /// Horizontal sampling interval in metres; rays advance by half of it.
    fn spacing_m(&self) -> f64;
}

/// First terrain point hit by the ray through a pixel.
///
/// The ray advances in half-cell steps until it passes below the surface,
/// then bisects that step. It fails when the ray rises, leaves the terrain
/// model, or exceeds the model's horizontal reach, rather than returning a
/// point on an extrapolated surface.
pub fn intersect(
    frame: &LocalFrame,
    terrain: &dyn Terrain,
    camera: &PinholeCamera,
    pose: &CameraPose,
    pixel: PlanarPoint,
) -> Result<GeodeticPoint, CameraError> {
    let direction = pose.ray(camera, pixel);
    let step = terrain.spacing_m() / 2.0;
    if !step.is_finite() || step <= 0.0 {
        return Err(CameraError::Invalid("terrain spacing must be positive"));
    }
    let clearance = |distance: f64| -> Option<f64> {
        let point = frame.to_geodetic(pose.centre + direction * distance);
        terrain
            .height_m(point.position)
            .map(|ground| point.height_m - ground)
    };
    let start = clearance(0.0).ok_or(CameraError::NoGround(pixel))?;
    if start <= 0.0 {
        return Err(CameraError::Invalid("camera is below the terrain surface"));
    }
    if direction.z >= 0.0 {
        return Err(CameraError::NoGround(pixel));
    }
    // Beyond this distance the ray would have to descend below sea level by
    // more than the camera's own height, so it cannot meet real terrain.
    let reach = (start + 12_000.0) / -direction.z;
    let mut previous = 0.0;
    let mut distance = step;
    while distance <= reach {
        let above = clearance(distance).ok_or(CameraError::NoGround(pixel))?;
        if above <= 0.0 {
            return refine(
                frame,
                pose.centre,
                direction,
                previous,
                distance,
                &clearance,
            );
        }
        previous = distance;
        distance += step;
    }
    Err(CameraError::NoGround(pixel))
}

fn refine(
    frame: &LocalFrame,
    centre: Vector3<f64>,
    direction: Vector3<f64>,
    mut above: f64,
    mut below: f64,
    clearance: &dyn Fn(f64) -> Option<f64>,
) -> Result<GeodeticPoint, CameraError> {
    for _ in 0..40 {
        let middle = (above + below) / 2.0;
        match clearance(middle) {
            Some(height) if height > 0.0 => above = middle,
            Some(_) => below = middle,
            None => {
                return Err(CameraError::Invalid(
                    "terrain sample missing during refinement",
                ));
            }
        }
    }
    Ok(frame.to_geodetic(centre + direction * below))
}
