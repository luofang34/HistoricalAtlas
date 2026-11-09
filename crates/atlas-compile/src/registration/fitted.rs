use super::RegistrationMethod;
use atlas_geo::registration::{
    AffineRegistration, ControlPoint, PlanarPoint, Registration, RegistrationError, ThinPlateSpline,
};

#[derive(Debug)]
pub(super) enum Fitted {
    Affine(AffineRegistration),
    ThinPlate(ThinPlateSpline),
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
        })
    }
}

impl Registration for Fitted {
    fn transform(&self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        match self {
            Self::Affine(model) => model.transform(point),
            Self::ThinPlate(model) => model.transform(point),
        }
    }
}
