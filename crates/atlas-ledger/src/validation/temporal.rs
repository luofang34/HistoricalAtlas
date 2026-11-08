use std::collections::BTreeMap;

use atlas_core::{
    assertion::{TemporalAssertion, TemporalValue},
    identity::{EntityId, FactId},
};

use super::{ValidationError, caused, fail};
use crate::model::{Entity, Estimate, Record};

pub(super) fn validate_temporal(
    estimate: &Estimate,
    assertion: &TemporalAssertion<TemporalValue>,
    entities: &BTreeMap<EntityId, &Entity>,
    records: &BTreeMap<FactId, &Record>,
) -> Result<(), ValidationError> {
    assertion
        .validity
        .validate()
        .map_err(|e| caused(&estimate.key, e))?;
    if assertion.validity.start.is_none() && assertion.validity.end.is_none() {
        return Err(fail(
            &estimate.key,
            "temporal estimate requires a dated endpoint",
        ));
    }
    if !estimate.basis.iter().any(|id| {
        records
            .get(id)
            .is_some_and(|record| record.entity_id == estimate.entity_id)
    }) {
        return Err(fail(
            &estimate.key,
            "temporal basis must include the subject's source record",
        ));
    }
    match &assertion.value {
        TemporalValue::Name(name) => {
            if name.text.trim().is_empty()
                || name
                    .language
                    .as_ref()
                    .is_some_and(|language| language.trim().is_empty())
            {
                return Err(fail(
                    &estimate.key,
                    "name text and supplied language must be nonempty",
                ));
            }
        }
        TemporalValue::Hierarchy(hierarchy) => {
            for id in hierarchy.parent_id.iter().chain(&hierarchy.seat_id) {
                validate_relation(&estimate.key, estimate.entity_id, *id, entities)?;
            }
        }
        TemporalValue::Demographics(demographics) => {
            if (demographics.households.is_none()
                && demographics.population.is_none()
                && demographics.ethnicities.is_empty())
                || demographics
                    .ethnicities
                    .iter()
                    .any(|label| label.trim().is_empty())
            {
                return Err(fail(
                    &estimate.key,
                    "demographics require a quantity or population-group label",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_relation(
    context: &str,
    subject: EntityId,
    target: EntityId,
    entities: &BTreeMap<EntityId, &Entity>,
) -> Result<(), ValidationError> {
    if subject == target
        || !entities
            .get(&subject)
            .zip(entities.get(&target))
            .is_some_and(|(a, b)| a.body == b.body)
    {
        return Err(fail(
            context,
            "unknown, self-referencing, or cross-body hierarchy relation",
        ));
    }
    Ok(())
}
