use super::{DerivationError, DerivationRequest, evidence};
use crate::input::LedgerInput;
use atlas_core::identity::FactId;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn resolve(
    input: &LedgerInput,
    request: &DerivationRequest,
    bases: &mut [BTreeSet<FactId>],
) -> Result<Vec<[usize; 2]>, DerivationError> {
    let subjects: BTreeMap<_, _> = request
        .units
        .iter()
        .enumerate()
        .map(|(i, u)| {
            evidence::checked(input, u.record_id, &request.source_id).map(|r| (r.entity_id, i))
        })
        .collect::<Result<_, _>>()?;
    let mut pairs = BTreeSet::new();
    for (index, unit) in request.units.iter().enumerate() {
        let Some(constraint) = &unit.area_constraint else {
            continue;
        };
        let record = evidence::checked(input, constraint.record_id, &request.source_id)?;
        let mut unresolved = BTreeSet::new();
        for review in &constraint.unresolved_neighbors {
            review.validate(&input.data, record.id)?;
            if !unresolved.insert(review.entity_id) || !subjects.contains_key(&review.entity_id) {
                return Err(DerivationError::Evidence(
                    "unresolved adjacencies need distinct reconstructed neighbors".into(),
                ));
            }
        }
        let Some(territory) = &record.observation.territory else {
            continue;
        };
        for (entity, other) in territory.neighbors.iter().filter_map(|n| {
            n.entity_id
                .and_then(|id| subjects.get(&id).map(|other| (id, *other)))
        }) {
            let review = constraint
                .unresolved_neighbors
                .iter()
                .find(|r| r.entity_id == entity);
            if review.is_none() {
                pairs.insert([index.min(other), index.max(other)]);
            }
            for affected in [index, other] {
                let basis = bases.get_mut(affected).ok_or_else(|| {
                    DerivationError::Evidence("neighbor constraint has no unit basis".into())
                })?;
                basis.insert(record.id);
                if let Some(review) = review {
                    basis.extend(&review.basis);
                }
            }
        }
    }
    Ok(pairs.into_iter().collect())
}

#[cfg(test)]
mod tests;
