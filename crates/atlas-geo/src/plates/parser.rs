use super::{PlateId, PlateModelError, RotationSample, RotationSequence};
use crate::{coordinates::LonLat, rotation::EulerRotation};

pub(super) fn parse(text: &str) -> Result<Vec<RotationSequence>, PlateModelError> {
    let mut sequences: Vec<RotationSequence> = Vec::new();
    for (offset, text) in text.lines().enumerate() {
        let text = text.trim();
        if text.is_empty() || text.starts_with(['!', '#']) {
            continue;
        }
        let line = offset.wrapping_add(1);
        let mut columns = text.split_whitespace();
        let moving_plate = identifier(columns.next(), line, "moving plate")?;
        if moving_plate == PlateId(999) {
            continue;
        }
        let age_ma = number(columns.next(), line, "age")?;
        let latitude = number(columns.next(), line, "pole latitude")?;
        let longitude = number(columns.next(), line, "pole longitude")?;
        let angle_degrees = number(columns.next(), line, "angle")?;
        let fixed_plate = identifier(columns.next(), line, "fixed plate")?;
        let description = columns.collect::<Vec<_>>().join(" ");
        let sample = RotationSample {
            age_ma,
            rotation: EulerRotation {
                pole: LonLat {
                    longitude: if (-180.0..=180.0).contains(&longitude) {
                        longitude
                    } else {
                        (longitude + 180.0).rem_euclid(360.0) - 180.0
                    },
                    latitude,
                },
                angle_degrees,
            },
            source_line: line,
            description: description.trim_start_matches('!').trim().to_owned(),
        };
        if let Some(sequence) = sequences.last_mut().filter(|sequence| {
            sequence.moving_plate == moving_plate && sequence.fixed_plate == fixed_plate
        }) {
            sequence.samples.push(sample);
        } else {
            sequences.push(RotationSequence {
                moving_plate,
                fixed_plate,
                samples: vec![sample],
            });
        }
    }
    Ok(sequences)
}

fn identifier(
    text: Option<&str>,
    line: usize,
    field: &'static str,
) -> Result<PlateId, PlateModelError> {
    text.ok_or(PlateModelError::Columns { line })?
        .parse()
        .map(PlateId)
        .map_err(|source| PlateModelError::Identifier {
            line,
            field,
            source,
        })
}

fn number(text: Option<&str>, line: usize, field: &'static str) -> Result<f64, PlateModelError> {
    text.ok_or(PlateModelError::Columns { line })?
        .parse()
        .map_err(|source| PlateModelError::Number {
            line,
            field,
            source,
        })
}
