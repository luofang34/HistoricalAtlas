use super::{
    RegistrationJob, RegistrationJobError, RegistrationMethod,
    camera::{CameraReport, FittedCamera},
};
use atlas_geo::registration::{
    AffineRegistration, ControlPoint, ControlSupport, PlanarPoint, ProjectiveRegistration,
    Registration, RegistrationError, ThinPlateSpline,
};

#[derive(Debug)]
pub(super) enum Fitted {
    Affine(AffineRegistration),
    ThinPlate(ThinPlateSpline),
    /// Projective fits are only defined between their controls, so evaluation
    /// outside the control hull is refused rather than extrapolated toward the horizon.
    Projective(ProjectiveRegistration, ControlSupport),
    Camera(Box<FittedCamera>),
}

impl Fitted {
    pub(super) fn fit(
        input: &crate::input::LedgerInput,
        job: &RegistrationJob,
        controls: &[ControlPoint],
    ) -> Result<Self, RegistrationJobError> {
        Ok(match job.method {
            RegistrationMethod::Affine => Self::Affine(AffineRegistration::fit(controls)?),
            RegistrationMethod::ThinPlate { smoothing } => {
                Self::ThinPlate(ThinPlateSpline::fit(controls, smoothing)?)
            }
            RegistrationMethod::Projective => Self::Projective(
                ProjectiveRegistration::fit(controls)?,
                ControlSupport::new(controls)?,
            ),
            RegistrationMethod::Camera { focal_px } => {
                Self::Camera(Box::new(FittedCamera::fit(input, job, focal_px)?))
            }
        })
    }

    pub(super) fn camera_report(&self) -> Option<&CameraReport> {
        match self {
            Self::Camera(camera) => Some(&camera.report),
            _ => None,
        }
    }
}

impl Registration for Fitted {
    fn transform(&self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        match self {
            Self::Affine(model) => model.transform(point),
            Self::ThinPlate(model) => model.transform(point),
            Self::Projective(model, support) => {
                support.require(point)?;
                model.transform(point)
            }
            Self::Camera(camera) => camera.transform(point),
        }
    }
}
