use std::collections::BTreeMap;

use atlas_core::{assertion::TemporalValue, identity::EntityId, time::EpochTime};

use super::{ValidationError, fail};
use crate::model::{Dataset, Estimate, EstimateValue};

#[derive(Clone, Copy)]
struct Window {
    start: Option<EpochTime>,
    end: Option<EpochTime>,
}

impl Window {
    fn intersect(self, other: Self) -> Option<Self> {
        let start = self.start.into_iter().chain(other.start).max();
        let end = self.end.into_iter().chain(other.end).min();
        if start.zip(end).is_some_and(|(start, end)| start >= end) {
            None
        } else {
            Some(Self { start, end })
        }
    }
}

pub(super) fn validate_hierarchy(data: &Dataset) -> Result<(), ValidationError> {
    validate_hierarchy_slices(&data.estimates, &[])
}

pub(super) fn validate_hierarchy_slices(
    existing: &[Estimate],
    added: &[Estimate],
) -> Result<(), ValidationError> {
    let mut edges: BTreeMap<EntityId, Vec<(EntityId, Window, &str)>> = BTreeMap::new();
    for estimate in existing.iter().chain(added) {
        if let EstimateValue::Temporal { assertion } = &estimate.value
            && let TemporalValue::Hierarchy(hierarchy) = &assertion.value
            && let Some(parent) = hierarchy.parent_id
        {
            let window = Window {
                start: assertion.validity.start.map(|value| value.nominal),
                end: assertion.validity.end.map(|value| value.nominal),
            };
            edges
                .entry(estimate.entity_id)
                .or_default()
                .push((parent, window, &estimate.key));
        }
    }
    for (subject, relations) in &edges {
        for (parent, window, key) in relations {
            let mut pending = vec![(*parent, *window, vec![*subject])];
            while let Some((node, window, mut path)) = pending.pop() {
                if path.contains(&node) {
                    return Err(fail(
                        *key,
                        format!("concurrent nominal hierarchy cycle at {node:?}"),
                    ));
                }
                path.push(node);
                for (parent, next, _) in edges.get(&node).into_iter().flatten() {
                    if let Some(overlap) = window.intersect(*next) {
                        pending.push((*parent, overlap, path.clone()));
                    }
                }
            }
        }
    }
    Ok(())
}
