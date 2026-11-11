use super::{ValidationError, caused, fail};
use crate::model::{Dataset, Dispute, Entity, Estimate, EstimateValue, Record, Source};
use atlas_core::identity::{EntityId, FactId};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate_estimates(
    data: &Dataset,
    entities: &BTreeMap<EntityId, &Entity>,
    records: &BTreeMap<FactId, &Record>,
    sources: &BTreeMap<&str, &Source>,
) -> Result<(), ValidationError> {
    validate_added_estimates(
        &Dataset::default(),
        &data.estimates,
        &data.disputes,
        entities,
        records,
        sources,
    )
}

pub(super) fn validate_added_estimates<'a>(
    existing: &'a Dataset,
    added_estimates: &'a [Estimate],
    added_disputes: &'a [Dispute],
    entities: &BTreeMap<EntityId, &Entity>,
    records: &BTreeMap<FactId, &Record>,
    sources: &BTreeMap<&str, &Source>,
) -> Result<(), ValidationError> {
    let mut estimates: BTreeMap<FactId, &Estimate> =
        existing.estimates.iter().map(|e| (e.id, e)).collect();
    for estimate in added_estimates {
        let expected = FactId::from_key(&estimate.key).map_err(|e| caused(&estimate.key, e))?;
        if estimate.id != expected
            || records.contains_key(&estimate.id)
            || estimates.insert(estimate.id, estimate).is_some()
        {
            return Err(fail(
                &estimate.key,
                "duplicate or noncanonical estimate identity",
            ));
        }
        if !entities.contains_key(&estimate.entity_id) || estimate.brief.trim().is_empty() {
            return Err(fail(
                &estimate.key,
                "estimate requires an entity and reasoning",
            ));
        }
        if estimate.basis.is_empty()
            || estimate
                .basis
                .iter()
                .any(|id| !records.get(id).is_some_and(|record| record.checked))
        {
            return Err(fail(
                &estimate.key,
                "estimate basis requires checked source records",
            ));
        }
        validate_value(estimate, entities, records, sources)?;
    }
    for dispute in added_disputes {
        let unique: BTreeSet<_> = dispute.alternatives.iter().collect();
        let selected = estimates.get(&dispute.estimate_id);
        if unique.len() < 2
            || dispute.brief.trim().is_empty()
            || !selected.is_some_and(|e| e.entity_id == dispute.entity_id)
            || dispute.alternatives.iter().any(|id| {
                !records
                    .get(id)
                    .is_some_and(|r| r.entity_id == dispute.entity_id && r.checked)
            })
        {
            return Err(fail(
                format!("{:?}", dispute.entity_id),
                "dispute requires distinct checked claims and a matching estimate",
            ));
        }
    }
    Ok(())
}

fn validate_value(
    estimate: &Estimate,
    entities: &BTreeMap<EntityId, &Entity>,
    records: &BTreeMap<FactId, &Record>,
    sources: &BTreeMap<&str, &Source>,
) -> Result<(), ValidationError> {
    match &estimate.value {
        EstimateValue::Plate {
            model_id,
            plate_id,
            validity,
        } => {
            if let Some(span) = validity {
                span.validate().map_err(|e| caused(&estimate.key, e))?;
                if span.start.is_none() && span.end.is_none() {
                    return Err(fail(&estimate.key, "plate validity requires an endpoint"));
                }
            }
            let on_earth = entities
                .get(&estimate.entity_id)
                .is_some_and(|entity| entity.body == atlas_geo::body::BodyId::Earth);
            let subject_basis = estimate.basis.iter().any(|id| {
                records
                    .get(id)
                    .is_some_and(|record| record.entity_id == estimate.entity_id)
            });
            if !on_earth
                || !subject_basis
                || model_id.trim().is_empty()
                || matches!(plate_id.0, 0 | 999)
            {
                return Err(fail(
                    &estimate.key,
                    "plate assignment needs an Earth subject record, a model, and an active plate ID",
                ));
            }
        }
        EstimateValue::Origin { time } => {
            time.validate().map_err(|e| caused(&estimate.key, e))?;
            if !estimate.basis.iter().any(|id| {
                records
                    .get(id)
                    .is_some_and(|record| record.entity_id == estimate.entity_id)
            }) {
                return Err(fail(
                    &estimate.key,
                    "origin basis must include the subject's source record",
                ));
            }
        }
        EstimateValue::Location {
            point,
            datum,
            uncertainty_m,
        } => {
            location(estimate, entities, *point, *datum, *uncertainty_m)?;
            if *datum == atlas_geo::coordinates::Datum::Unspecified {
                return Err(fail(
                    &estimate.key,
                    "location estimate needs a known datum and positive uncertainty",
                ));
            }
        }
        EstimateValue::MapLocation {
            record_id,
            point,
            datum,
            uncertainty_m,
            registration,
        } => {
            location(estimate, entities, *point, *datum, *uncertainty_m)?;
            map_location(estimate, records, sources, *record_id, registration)?;
        }
        EstimateValue::Temporal { assertion } => {
            super::temporal::validate_temporal(estimate, assertion, entities, records)?;
        }
    }
    Ok(())
}

fn map_location(
    estimate: &Estimate,
    records: &BTreeMap<FactId, &Record>,
    sources: &BTreeMap<&str, &Source>,
    record_id: FactId,
    registration: &str,
) -> Result<(), ValidationError> {
    let name = registration
        .strip_prefix("maps/")
        .and_then(|p| p.strip_suffix(".json"));
    let valid_record = records.get(&record_id).is_some_and(|r| {
        if r.entity_id != estimate.entity_id {
            return false;
        }
        if r.scan_page.is_some() {
            return true;
        }
        sources.get(r.source_id.as_str()).is_some_and(|source| {
            source.scan_pages.is_none()
                && source
                    .capture
                    .as_ref()
                    .and_then(|capture| capture.sha256.as_ref())
                    .is_some()
        })
    });
    if name.is_none_or(|n| {
        n.is_empty()
            || !n
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    }) || !estimate.basis.contains(&record_id)
        || !valid_record
    {
        return Err(fail(
            &estimate.key,
            "map location requires its subject sheet record and a registration artifact",
        ));
    }
    Ok(())
}

fn location(
    estimate: &Estimate,
    entities: &BTreeMap<EntityId, &Entity>,
    point: atlas_geo::coordinates::LonLat,
    datum: atlas_geo::coordinates::Datum,
    uncertainty_m: f64,
) -> Result<(), ValidationError> {
    let entity = entities
        .get(&estimate.entity_id)
        .ok_or_else(|| fail(&estimate.key, "unknown entity"))?;
    point.validate().map_err(|e| caused(&estimate.key, e))?;
    datum
        .validate_body(entity.body)
        .map_err(|e| caused(&estimate.key, e))?;
    if !uncertainty_m.is_finite() || uncertainty_m <= 0.0 {
        return Err(fail(
            &estimate.key,
            "location estimate needs positive uncertainty",
        ));
    }
    Ok(())
}
