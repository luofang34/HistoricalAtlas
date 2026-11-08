//! Time-slice selection retaining uncertainty, competing values, and evidence links.

use std::{collections::BTreeMap, str::FromStr};

use atlas_core::{
    assertion::{TemporalAssertion, TemporalProperty, TemporalValue},
    identity::{EntityId, FactId},
    time::{EpochTime, TimeError, TimeMembership},
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    model::{Dataset, Dispute, Estimate, EstimateValue, SourceCatalog},
    validation::{ValidationError, validate},
};

/// Which endpoint interpretation controls inclusion in a time slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeSelection {
    /// Include values at their nominal endpoints, retaining the uncertainty label.
    Nominal,
    /// Include every value that may apply at the queried time.
    Possible,
    /// Include values supported by every allowed pair of endpoints.
    Certain,
}

impl FromStr for TimeSelection {
    type Err = SelectionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "nominal" => Ok(Self::Nominal),
            "possible" => Ok(Self::Possible),
            "certain" => Ok(Self::Certain),
            _ => Err(SelectionError(value.to_owned())),
        }
    }
}

/// Unknown time-slice selection mode.
#[derive(Debug, Error)]
#[error("unknown time selection {0:?}; choose nominal, possible, or certain")]
pub struct SelectionError(pub String);

/// One selected interpretation with its source-record links.
#[derive(Debug, Serialize)]
pub struct SelectedAssertion<'a> {
    /// Permanent estimate identity.
    pub estimate_id: FactId,
    /// Checked source records supporting the interpretation.
    pub basis: &'a [FactId],
    /// Editorial explanation.
    pub brief: &'a str,
    /// Property and complete endpoint uncertainty.
    pub assertion: &'a TemporalAssertion<TemporalValue>,
    /// Support for this instant independent of the selection mode.
    pub membership: TimeMembership,
    /// Source disagreements for which this estimate is the selected interpretation.
    pub disputes: Vec<&'a Dispute>,
}

/// All supported interpretations of a single property slot.
#[derive(Debug, Serialize)]
pub struct PropertySlice<'a> {
    /// Property and, for names, linguistic role and language.
    pub property: TemporalProperty,
    /// Whether the selected interpretations contain different values.
    pub conflicting: bool,
    /// Candidates in permanent-identity order.
    pub candidates: Vec<SelectedAssertion<'a>>,
}

/// Selected temporal properties for one entity.
#[derive(Debug, Serialize)]
pub struct EntitySlice<'a> {
    /// Subject entity.
    pub entity_id: EntityId,
    /// Populated property slots in stable order.
    pub properties: Vec<PropertySlice<'a>>,
}

/// Reproducible time query, including its uncertainty-selection mode.
#[derive(Debug, Serialize)]
pub struct TimeSlice<'a> {
    /// Queried TT millisecond instant.
    pub at: EpochTime,
    /// Endpoint-selection mode.
    pub selection: TimeSelection,
    /// Entities with temporal interpretations at this instant.
    pub entities: Vec<EntitySlice<'a>>,
}

/// Validated temporal interpretations reusable across slider queries.
pub struct Timeline<'a> {
    estimates: Vec<&'a Estimate>,
    disputes: BTreeMap<FactId, Vec<&'a Dispute>>,
}

impl<'a> Timeline<'a> {
    /// Validate the ledger and retain temporal estimates in stable identity order.
    pub fn new(catalog: &SourceCatalog, data: &'a Dataset) -> Result<Self, ValidationError> {
        validate(catalog, data)?;
        let mut estimates: Vec<_> = data
            .estimates
            .iter()
            .filter(|estimate| matches!(estimate.value, EstimateValue::Temporal { .. }))
            .collect();
        estimates.sort_by_key(|estimate| estimate.id);
        let mut disputes: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for dispute in &data.disputes {
            disputes
                .entry(dispute.estimate_id)
                .or_default()
                .push(dispute);
        }
        Ok(Self {
            estimates,
            disputes,
        })
    }

    /// Select supported properties without choosing between competing values.
    pub fn at(&self, at: EpochTime, selection: TimeSelection) -> Result<TimeSlice<'a>, TimeError> {
        let mut entities: BTreeMap<EntityId, BTreeMap<TemporalProperty, Vec<_>>> = BTreeMap::new();
        for estimate in &self.estimates {
            let EstimateValue::Temporal { assertion } = &estimate.value else {
                continue;
            };
            let membership = assertion.validity.membership(at)?;
            let included = match selection {
                TimeSelection::Nominal => assertion.validity.nominally_contains(at)?,
                TimeSelection::Possible => matches!(
                    membership,
                    TimeMembership::Possible | TimeMembership::Certain
                ),
                TimeSelection::Certain => membership == TimeMembership::Certain,
            };
            if included {
                entities
                    .entry(estimate.entity_id)
                    .or_default()
                    .entry(assertion.value.property())
                    .or_default()
                    .push(SelectedAssertion {
                        estimate_id: estimate.id,
                        basis: &estimate.basis,
                        brief: &estimate.brief,
                        assertion,
                        membership,
                        disputes: self.disputes.get(&estimate.id).cloned().unwrap_or_default(),
                    });
            }
        }
        let entities = entities
            .into_iter()
            .map(|(entity_id, properties)| EntitySlice {
                entity_id,
                properties: properties
                    .into_iter()
                    .map(|(property, candidates)| {
                        let conflicting = candidates.first().is_some_and(|first| {
                            candidates
                                .iter()
                                .any(|candidate| candidate.assertion.value != first.assertion.value)
                        });
                        PropertySlice {
                            property,
                            conflicting,
                            candidates,
                        }
                    })
                    .collect(),
            })
            .collect();
        Ok(TimeSlice {
            at,
            selection,
            entities,
        })
    }
}

#[cfg(test)]
mod tests;
