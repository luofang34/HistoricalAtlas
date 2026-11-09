use nalgebra::{DMatrix, DVector, Matrix3, Rotation3, Vector3};
use serde::{Deserialize, Serialize};

use super::{CameraError, CameraPose, PinholeCamera};
use crate::registration::{ControlPoint, PlanarPoint, ProjectiveRegistration};

/// Largest accepted relative focal-length uncertainty per pixel of control error.
///
/// Near-vertical views trade focal length against height almost freely, so
/// controls can be fitted to a fraction of a pixel by a camera several times
/// too high; such fits are refused unless the focal length is supplied.
const MAXIMUM_FOCAL_SPREAD_PER_PX: f64 = 0.01;

/// Pixel residual assigned to a control behind the camera, large enough to
/// dominate any real misfit without overflowing the normal equations.
const BEHIND_CAMERA_PX: f64 = 1e6;

/// A measured pixel and its terrain position in the local frame.
#[derive(Debug, Clone, Copy)]
pub struct CameraControl {
    /// Image location.
    pub pixel: PlanarPoint,
    /// Ground location in local east-north-up metres, including terrain height.
    pub world: Vector3<f64>,
}

/// Image geometry and optional fixed focal length.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResectionOptions {
    /// Principal point; the image centre unless the camera is calibrated.
    pub principal: PlanarPoint,
    /// Fixed focal length in pixels; estimated from the controls when absent.
    pub focal_px: Option<f64>,
}

/// A fitted camera and its reprojection error at the fitting controls.
#[derive(Debug, Clone)]
pub struct CameraFit {
    /// Intrinsics, with the estimated or fixed focal length.
    pub camera: PinholeCamera,
    /// Optical centre and orientation.
    pub pose: CameraPose,
    /// Per-control reprojection error in pixels, in input order.
    pub residuals_px: Vec<f64>,
    /// Relative standard deviation of an estimated focal length for one pixel of
    /// independent control error; absent when the focal length was fixed.
    pub focal_spread_per_px: Option<f64>,
}

/// Estimate a pinhole camera from four or more ground controls.
///
/// A homography between the ground and the image, which is exact for level
/// ground, supplies the focal length and pose; Levenberg-Marquardt then fits
/// all controls with their terrain heights. Near-vertical photographs leave
/// focal length and height interchangeable, so they need `focal_px`.
pub fn resect(
    controls: &[CameraControl],
    options: &ResectionOptions,
) -> Result<CameraFit, CameraError> {
    if options
        .focal_px
        .is_some_and(|focal| !focal.is_finite() || focal <= 0.0)
    {
        return Err(CameraError::Invalid("focal length must be positive"));
    }
    options.principal.validate()?;
    let (focal, pose) = initialize(controls, options)?;
    let fixed = options.focal_px;
    let start: Vec<f64> = [0.0; 3]
        .into_iter()
        .chain(pose.centre.iter().copied())
        .chain(fixed.is_none().then(|| focal.ln()))
        .collect();
    let model = |parameters: &[f64]| unpack(parameters, &pose, fixed, options.principal);
    let residual = |parameters: &[f64]| {
        let (camera, pose) = model(parameters);
        reprojection(controls, &camera, &pose)
    };
    let solved = levenberg_marquardt(DVector::from_vec(start), &residual)?;
    let focal_spread_per_px = match fixed {
        Some(_) => None,
        None => {
            let spread = focal_spread(&solved, &residual);
            if spread.is_none_or(|value| value > MAXIMUM_FOCAL_SPREAD_PER_PX) {
                return Err(CameraError::FocalUnresolved);
            }
            spread
        }
    };
    let (camera, pose) = model(solved.as_slice());
    let residuals_px: Vec<f64> = controls
        .iter()
        .map(|control| {
            pose.project(&camera, control.world)
                .map(|pixel| pixel.distance(control.pixel))
        })
        .collect::<Option<_>>()
        .ok_or(CameraError::NoConvergence(
            "a control lies behind the camera",
        ))?;
    Ok(CameraFit {
        camera,
        pose,
        residuals_px,
        focal_spread_per_px,
    })
}

fn unpack(
    parameters: &[f64],
    start: &CameraPose,
    fixed: Option<f64>,
    principal: PlanarPoint,
) -> (PinholeCamera, CameraPose) {
    let value = |index: usize| parameters.get(index).copied().unwrap_or(0.0);
    let correction = Rotation3::new(Vector3::new(value(0), value(1), value(2)));
    let focal_px = fixed.unwrap_or_else(|| value(6).exp());
    (
        PinholeCamera {
            focal_px,
            principal,
        },
        CameraPose {
            centre: Vector3::new(value(3), value(4), value(5)),
            world_to_camera: start.world_to_camera * correction,
        },
    )
}

fn reprojection(
    controls: &[CameraControl],
    camera: &PinholeCamera,
    pose: &CameraPose,
) -> DVector<f64> {
    DVector::from_iterator(
        2 * controls.len(),
        controls.iter().flat_map(|control| {
            pose.project(camera, control.world)
                .map_or([BEHIND_CAMERA_PX; 2], |pixel| {
                    [pixel.x - control.pixel.x, pixel.y - control.pixel.y]
                })
        }),
    )
}

/// Focal length and pose from the ground-to-image homography of the controls' mean plane.
fn initialize(
    controls: &[CameraControl],
    options: &ResectionOptions,
) -> Result<(f64, CameraPose), CameraError> {
    let planar: Vec<_> = controls
        .iter()
        .map(|control| ControlPoint {
            source: PlanarPoint {
                x: control.world.x,
                y: control.world.y,
            },
            target: control.pixel,
        })
        .collect();
    let homography = ProjectiveRegistration::fit(&planar)?.matrix();
    let centred = Matrix3::new(
        1.0,
        0.0,
        -options.principal.x,
        0.0,
        1.0,
        -options.principal.y,
        0.0,
        0.0,
        1.0,
    ) * homography;
    let focal = match options.focal_px {
        Some(focal) => focal,
        None => estimate_focal(&centred).ok_or(CameraError::FocalUnresolved)?,
    };
    let height = controls.iter().map(|c| c.world.z).sum::<f64>() / controls.len() as f64;
    let pose = decompose(&centred, focal, height, controls)?;
    Ok((focal, pose))
}

/// Zhang's constraints on the first two columns of `K [r1 r2 t]`.
fn estimate_focal(homography: &Matrix3<f64>) -> Option<f64> {
    let (a, b) = (homography.column(0), homography.column(1));
    let orthogonal = -(a.x * b.x + a.y * b.y) / (a.z * b.z);
    let equal_norm =
        (a.x.powi(2) + a.y.powi(2) - b.x.powi(2) - b.y.powi(2)) / (b.z.powi(2) - a.z.powi(2));
    let estimates: Vec<f64> = [orthogonal, equal_norm]
        .into_iter()
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(f64::sqrt)
        .collect();
    (!estimates.is_empty()).then(|| estimates.iter().sum::<f64>() / estimates.len() as f64)
}

fn decompose(
    homography: &Matrix3<f64>,
    focal: f64,
    height: f64,
    controls: &[CameraControl],
) -> Result<CameraPose, CameraError> {
    let inverse_k = Matrix3::new(1.0 / focal, 0.0, 0.0, 0.0, 1.0 / focal, 0.0, 0.0, 0.0, 1.0);
    let scaled = inverse_k * homography;
    let (c1, c2, c3) = (scaled.column(0), scaled.column(1), scaled.column(2));
    let mut lambda = 2.0 / (c1.norm() + c2.norm());
    // The ground must lie in front of the camera.
    let depth =
        |control: &CameraControl| lambda * (c1.z * control.world.x + c2.z * control.world.y + c3.z);
    if controls.iter().map(depth).sum::<f64>() < 0.0 {
        lambda = -lambda;
    }
    let r1 = c1 * lambda;
    let r2 = c2 * lambda;
    let raw = Matrix3::from_columns(&[r1, r2, r1.cross(&r2)]);
    let svd = raw.svd(true, true);
    let (Some(u), Some(v_t)) = (svd.u, svd.v_t) else {
        return Err(CameraError::NoConvergence("rotation orthonormalization"));
    };
    let mut rotation = u * v_t;
    if rotation.determinant() < 0.0 {
        rotation = -rotation;
    }
    let world_to_camera = Rotation3::from_matrix_unchecked(rotation);
    // Plane coordinates were measured at the mean control height.
    let translation = c3 * lambda - world_to_camera * Vector3::new(0.0, 0.0, height);
    Ok(CameraPose {
        centre: -(world_to_camera.inverse() * translation),
        world_to_camera,
    })
}

fn levenberg_marquardt(
    start: DVector<f64>,
    residual: &dyn Fn(&[f64]) -> DVector<f64>,
) -> Result<DVector<f64>, CameraError> {
    let mut parameters = start;
    let mut current = residual(parameters.as_slice());
    let mut cost = current.norm_squared();
    let mut damping = 1e-3;
    for _ in 0..200 {
        let jacobian = jacobian(&parameters, &current, residual);
        let normal = jacobian.transpose() * &jacobian;
        let gradient = jacobian.transpose() * &current;
        let mut improved = false;
        while damping < 1e12 {
            let mut system = normal.clone();
            for index in 0..system.nrows() {
                if let Some(value) = system.get_mut((index, index)) {
                    *value += damping * value.abs().max(1e-12);
                }
            }
            let Some(step) = system.lu().solve(&(-&gradient)) else {
                damping *= 10.0;
                continue;
            };
            let candidate = &parameters + &step;
            let values = residual(candidate.as_slice());
            let candidate_cost = values.norm_squared();
            if candidate_cost.is_finite() && candidate_cost < cost {
                let converged = cost - candidate_cost <= 1e-14 * cost.max(1e-30);
                parameters = candidate;
                current = values;
                cost = candidate_cost;
                damping = (damping / 3.0).max(1e-12);
                improved = !converged;
                break;
            }
            damping *= 4.0;
        }
        if !improved {
            break;
        }
    }
    if cost.is_finite() && current.iter().all(|value| value.abs() < BEHIND_CAMERA_PX) {
        Ok(parameters)
    } else {
        Err(CameraError::NoConvergence("controls behind the camera"))
    }
}

/// Standard deviation of log focal length from the Gauss-Newton covariance at unit pixel noise.
fn focal_spread(
    parameters: &DVector<f64>,
    residual: &dyn Fn(&[f64]) -> DVector<f64>,
) -> Option<f64> {
    let current = residual(parameters.as_slice());
    let jacobian = jacobian(parameters, &current, residual);
    let covariance = (jacobian.transpose() * jacobian).try_inverse()?;
    let last = parameters.len().checked_sub(1)?;
    covariance
        .get((last, last))
        .copied()
        .filter(|variance| variance.is_finite() && *variance >= 0.0)
        .map(f64::sqrt)
}

fn jacobian(
    parameters: &DVector<f64>,
    current: &DVector<f64>,
    residual: &dyn Fn(&[f64]) -> DVector<f64>,
) -> DMatrix<f64> {
    let mut columns = Vec::with_capacity(parameters.len());
    for index in 0..parameters.len() {
        let mut shifted = parameters.clone();
        let step = 1e-6
            * parameters
                .get(index)
                .map_or(1.0, |value| value.abs().max(1.0));
        if let Some(value) = shifted.get_mut(index) {
            *value += step;
        }
        columns.push((residual(shifted.as_slice()) - current) / step);
    }
    DMatrix::from_columns(&columns)
}
