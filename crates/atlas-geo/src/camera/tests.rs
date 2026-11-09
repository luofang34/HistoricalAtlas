use super::*;
use crate::{
    coordinates::LonLat,
    geodesy::{GeodeticPoint, LocalFrame},
};
use nalgebra::Matrix3;

#[cfg(test)]
const ORIGIN: LonLat = LonLat {
    longitude: 160.0547,
    latitude: -9.4281,
};

/// Coastal plain rising to a 120 m hill about 2 km south of the origin.
#[cfg(test)]
struct Hill {
    frame: LocalFrame,
}

impl Terrain for Hill {
    fn height_m(&self, position: LonLat) -> Option<f64> {
        let local = self.frame.to_local(GeodeticPoint {
            position,
            height_m: 0.0,
        });
        (local.x.abs() < 8000.0 && local.y.abs() < 8000.0).then(|| {
            let distance = (local.x - 300.0).hypot(local.y + 2000.0);
            10.0 + 120.0 * (-(distance / 700.0).powi(2)).exp()
        })
    }

    fn spacing_m(&self) -> f64 {
        30.0
    }
}

#[cfg(test)]
fn scene() -> (LocalFrame, Hill) {
    let frame = LocalFrame::new(GeodeticPoint {
        position: ORIGIN,
        height_m: 0.0,
    });
    (frame, Hill { frame })
}

#[cfg(test)]
fn looking(centre: Vector3<f64>, heading_deg: f64, depression_deg: f64) -> CameraPose {
    let (h, d) = (heading_deg.to_radians(), depression_deg.to_radians());
    let forward = Vector3::new(h.sin() * d.cos(), h.cos() * d.cos(), -d.sin());
    let right = Vector3::new(h.cos(), -h.sin(), 0.0);
    let down = forward.cross(&right);
    CameraPose {
        centre,
        world_to_camera: Rotation3::from_matrix_unchecked(Matrix3::from_rows(&[
            right.transpose(),
            down.transpose(),
            forward.transpose(),
        ])),
    }
}

#[cfg(test)]
fn camera() -> PinholeCamera {
    PinholeCamera {
        focal_px: 1500.0,
        principal: PlanarPoint { x: 999.5, y: 749.5 },
    }
}

/// Ground controls seen by the camera, with heights taken from the terrain.
#[cfg(test)]
fn controls(frame: &LocalFrame, hill: &Hill, pose: &CameraPose) -> Vec<CameraControl> {
    [
        (200.0, 300.0),
        (1800.0, 250.0),
        (1700.0, 1350.0),
        (300.0, 1400.0),
        (1000.0, 800.0),
        (600.0, 1000.0),
    ]
    .into_iter()
    .map(|(x, y)| {
        let pixel = PlanarPoint { x, y };
        let ground = intersect(frame, hill, &camera(), pose, pixel).unwrap();
        CameraControl {
            pixel,
            world: frame.to_local(ground),
        }
    })
    .collect()
}

#[test]
fn oblique_resection_over_relief_recovers_focal_length_pose_and_ground_points() {
    let (frame, hill) = scene();
    let truth = looking(Vector3::new(-1500.0, 1200.0, 650.0), 150.0, 32.0);
    let controls = controls(&frame, &hill, &truth);
    let options = ResectionOptions {
        principal: camera().principal,
        focal_px: None,
    };
    let fit = resect(&controls, &options).unwrap();
    assert!(
        (fit.camera.focal_px - 1500.0).abs() < 0.5,
        "{}",
        fit.camera.focal_px
    );
    assert!(
        (fit.pose.centre - truth.centre).norm() < 0.5,
        "{:?}",
        fit.pose.centre
    );
    assert!(fit.residuals_px.iter().all(|error| *error < 1e-3));
    assert!(fit.focal_spread_per_px.unwrap() < 0.01);
    let angles = fit.pose.view_angles();
    assert!((angles.heading_deg - 150.0).abs() < 0.01);
    assert!((angles.depression_deg - 32.0).abs() < 0.01);
    let holdout = PlanarPoint {
        x: 1400.0,
        y: 600.0,
    };
    let expected = frame.to_local(intersect(&frame, &hill, &camera(), &truth, holdout).unwrap());
    let ground = intersect(&frame, &hill, &fit.camera, &fit.pose, holdout).unwrap();
    assert!((frame.to_local(ground) - expected).norm() < 1.0);
}

#[test]
fn sky_pixels_and_vertical_views_without_focal_length_are_rejected() {
    let (frame, hill) = scene();
    let oblique = looking(Vector3::new(-1500.0, 1200.0, 650.0), 150.0, 10.0);
    let sky = PlanarPoint { x: 1000.0, y: 0.0 };
    assert!(matches!(
        intersect(&frame, &hill, &camera(), &oblique, sky),
        Err(CameraError::NoGround(_))
    ));
    let vertical = looking(Vector3::new(0.0, -500.0, 3000.0), 0.0, 90.0);
    let controls = controls(&frame, &hill, &vertical);
    let unknown = ResectionOptions {
        principal: camera().principal,
        focal_px: None,
    };
    assert!(matches!(
        resect(&controls, &unknown),
        Err(CameraError::FocalUnresolved)
    ));
    let known = ResectionOptions {
        focal_px: Some(1500.0),
        ..unknown
    };
    let fit = resect(&controls, &known).unwrap();
    assert!((fit.pose.centre - vertical.centre).norm() < 0.5);
}
