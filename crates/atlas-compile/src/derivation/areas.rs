use super::{DerivationError, DerivationRequest, evidence};
use crate::input::LedgerInput;
use atlas_boundary::terrain::{AreaTarget, DerivedBoundary};
use atlas_core::identity::FactId;
use std::collections::BTreeSet;

pub(super) fn resolve(
    input: &LedgerInput,
    request: &DerivationRequest,
    bases: &mut [BTreeSet<FactId>],
) -> Result<Vec<AreaTarget>, DerivationError> {
    let mut targets = Vec::new();
    for (index, unit) in request.units.iter().enumerate() {
        let Some(constraint) = &unit.area_constraint else {
            continue;
        };
        let record = evidence::checked(input, constraint.record_id, &request.source_id)?;
        let subject = evidence::checked(input, unit.record_id, &request.source_id)?;
        let territory = record
            .observation
            .territory
            .as_ref()
            .filter(|_| record.entity_id == subject.entity_id)
            .ok_or_else(|| {
                DerivationError::Evidence(format!(
                    "area constraint {:?} needs territory evidence for unit {:?}",
                    constraint.record_id, unit.record_id
                ))
            })?;
        bases
            .get_mut(index)
            .ok_or_else(|| DerivationError::Evidence("area constraint has no unit basis".into()))?
            .insert(record.id);
        targets.push(AreaTarget {
            unit: index,
            square_km: territory.area_square_km,
            tolerance_square_km: constraint.tolerance_square_km,
        });
    }
    Ok(targets)
}

pub(super) fn annotate(
    request: &DerivationRequest,
    boundaries: &mut [DerivedBoundary],
) -> Result<(), DerivationError> {
    for unit in &request.units {
        let Some(constraint) = &unit.area_constraint else {
            continue;
        };
        let comparison = boundaries
            .iter_mut()
            .find(|b| b.record_id == unit.record_id)
            .and_then(|b| {
                b.comparisons
                    .iter_mut()
                    .find(|c| c.record_id == constraint.record_id)
            })
            .ok_or_else(|| {
                DerivationError::Evidence(format!(
                    "area constraint {:?} has no geometry comparison",
                    constraint.record_id
                ))
            })?;
        comparison.fitting_tolerance_square_km = Some(constraint.tolerance_square_km);
    }
    Ok(())
}
