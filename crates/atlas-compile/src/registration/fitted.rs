use super::RegistrationMethod;
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
}

impl Fitted {
    pub(super) fn fit(
        method: RegistrationMethod,
        controls: &[ControlPoint],
    ) -> Result<Self, RegistrationError> {
        Ok(match method {
            RegistrationMethod::Affine => Self::Affine(AffineRegistration::fit(controls)?),
            RegistrationMethod::ThinPlate { smoothing } => {
                Self::ThinPlate(ThinPlateSpline::fit(controls, smoothing)?)
            }
            RegistrationMethod::Projective => Self::Projective(
                ProjectiveRegistration::fit(controls)?,
                ControlSupport::new(controls)?,
            ),
        })
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
        }
    }
}
