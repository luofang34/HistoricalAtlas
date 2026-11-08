use std::collections::BTreeSet;

use super::{PlateId, PlateModelError, RotationModel, RotationStep};
use crate::rotation::EulerRotation;

impl RotationModel {
    pub(super) fn circuit(
        &self,
        mut plate: PlateId,
        age_ma: f64,
    ) -> Result<(EulerRotation, Vec<RotationStep>), PlateModelError> {
        let mut visited = BTreeSet::new();
        let mut rotation = EulerRotation::IDENTITY;
        let mut circuit = Vec::new();
        while plate != PlateId(0) {
            if !visited.insert(plate) {
                return Err(PlateModelError::Cycle { plate, age_ma });
            }
            // At a shared crossover, the older history owns the exact transition age.
            let sequence = self
                .sequences
                .get(&plate)
                .and_then(|histories| {
                    histories
                        .iter()
                        .rev()
                        .find(|history| history.start() <= age_ma && age_ma <= history.end())
                })
                .ok_or(PlateModelError::Missing { plate, age_ma })?;
            let step = sequence.at(age_ma)?;
            rotation = rotation.then(step.rotation)?;
            plate = step.fixed_plate;
            circuit.push(step);
        }
        Ok((rotation, circuit))
    }
}
