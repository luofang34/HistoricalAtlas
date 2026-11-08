use std::collections::BTreeMap;

use super::{PlateId, PlateModelError, RotationSequence, RotationStep};
use crate::rotation::EulerRotation;

pub(super) fn index(
    sequences: Vec<RotationSequence>,
) -> Result<BTreeMap<PlateId, Vec<RotationSequence>>, PlateModelError> {
    let mut index: BTreeMap<PlateId, Vec<RotationSequence>> = BTreeMap::new();
    for mut sequence in sequences {
        validate(&mut sequence)?;
        index
            .entry(sequence.moving_plate)
            .or_default()
            .push(sequence);
    }
    for (plate, histories) in &mut index {
        histories.sort_by(|left, right| left.start().total_cmp(&right.start()));
        for pair in histories.windows(2) {
            if let [younger, older] = pair
                && (younger.end() > older.start() || younger.start() == older.start())
            {
                return Err(PlateModelError::Sequence {
                    plate: *plate,
                    reason: "rotation histories overlap beyond a single crossover age".into(),
                });
            }
        }
    }
    Ok(index)
}

fn validate(sequence: &mut RotationSequence) -> Result<(), PlateModelError> {
    let invalid = |reason: &str| PlateModelError::Sequence {
        plate: sequence.moving_plate,
        reason: reason.into(),
    };
    if sequence.samples.is_empty() {
        return Err(invalid("rotation sequence is empty"));
    }
    if matches!(sequence.moving_plate.0, 0 | 999) || sequence.moving_plate == sequence.fixed_plate {
        return Err(invalid(
            "moving plate must differ from reserved IDs 0 and 999 and its fixed plate",
        ));
    }
    for sample in &sequence.samples {
        if !sample.age_ma.is_finite() || sample.age_ma < 0.0 || sample.source_line == 0 {
            return Err(invalid(
                "samples require nonnegative finite ages and one-based source lines",
            ));
        }
        sample
            .rotation
            .separation_degrees(EulerRotation::IDENTITY)
            .map_err(|source| PlateModelError::Sample {
                line: sample.source_line,
                source,
            })?;
    }
    sequence
        .samples
        .sort_by(|left, right| left.age_ma.total_cmp(&right.age_ma));
    for pair in sequence.samples.windows(2) {
        if let [first, last] = pair
            && first.age_ma == last.age_ma
            && first.rotation.separation_degrees(last.rotation)? > 1e-10
        {
            return Err(invalid("conflicting rotations at the same age"));
        }
    }
    Ok(())
}

impl RotationSequence {
    pub(super) fn start(&self) -> f64 {
        self.samples
            .first()
            .map_or(f64::NAN, |sample| sample.age_ma)
    }

    pub(super) fn end(&self) -> f64 {
        self.samples.last().map_or(f64::NAN, |sample| sample.age_ma)
    }

    pub(super) fn at(&self, age_ma: f64) -> Result<RotationStep, PlateModelError> {
        if let Some(sample) = self.samples.iter().find(|sample| sample.age_ma == age_ma) {
            return Ok(RotationStep {
                moving_plate: self.moving_plate,
                fixed_plate: self.fixed_plate,
                younger_line: sample.source_line,
                older_line: sample.source_line,
                rotation: sample.rotation,
            });
        }
        for pair in self.samples.windows(2) {
            if let [younger, older] = pair
                && younger.age_ma < age_ma
                && age_ma < older.age_ma
            {
                let fraction = (age_ma - younger.age_ma) / (older.age_ma - younger.age_ma);
                return Ok(RotationStep {
                    moving_plate: self.moving_plate,
                    fixed_plate: self.fixed_plate,
                    younger_line: younger.source_line,
                    older_line: older.source_line,
                    rotation: younger.rotation.interpolate(older.rotation, fraction)?,
                });
            }
        }
        Err(PlateModelError::Missing {
            plate: self.moving_plate,
            age_ma,
        })
    }
}
