use super::*;
use atlas_core::origin::OriginTime;

/// A formation interpretation with its evidence and occurrence constraint.
#[derive(Debug, Serialize)]
pub struct SelectedOrigin<'a> {
    /// Permanent estimate identity.
    pub estimate_id: FactId,
    /// Checked records supporting the formation interpretation.
    pub basis: &'a [FactId],
    /// Explanation of the age and its conversion to the TT axis.
    pub brief: &'a str,
    /// Full dating constraint.
    pub time: &'a OriginTime,
    /// Support that formation had occurred by the queried time.
    pub membership: TimeMembership,
    /// Source disagreements attached to this interpretation.
    pub disputes: Vec<&'a Dispute>,
}

pub(super) fn occurred(
    time: &OriginTime,
    at: EpochTime,
    selection: TimeSelection,
) -> Result<Option<TimeMembership>, TimeError> {
    let membership = time.occurred_by(at)?;
    let included = match selection {
        TimeSelection::Nominal => at >= time.nominal(),
        TimeSelection::Possible => membership != TimeMembership::Outside,
        TimeSelection::Certain => membership == TimeMembership::Certain,
    };
    Ok(included.then_some(membership))
}

pub(super) fn unformed(
    estimates: &[&Estimate],
    at: EpochTime,
    selection: TimeSelection,
) -> Result<BTreeSet<EntityId>, TimeError> {
    let mut states: BTreeMap<EntityId, bool> = BTreeMap::new();
    for estimate in estimates {
        let EstimateValue::Origin { time } = &estimate.value else {
            continue;
        };
        let formed = occurred(time, at, selection)?.is_some();
        let entry = states.entry(estimate.entity_id).or_insert(false);
        *entry |= formed;
    }
    Ok(states
        .into_iter()
        .filter_map(|(id, formed)| (!formed).then_some(id))
        .collect())
}

pub(super) fn merge<'a>(
    entities: &mut Vec<EntitySlice<'a>>,
    estimates: &[&'a Estimate],
    disputes: &BTreeMap<FactId, Vec<&'a Dispute>>,
    at: EpochTime,
    selection: TimeSelection,
) -> Result<(), TimeError> {
    let mut origins: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for estimate in estimates {
        let EstimateValue::Origin { time } = &estimate.value else {
            continue;
        };
        if let Some(membership) = occurred(time, at, selection)? {
            origins
                .entry(estimate.entity_id)
                .or_default()
                .push(SelectedOrigin {
                    estimate_id: estimate.id,
                    basis: &estimate.basis,
                    brief: &estimate.brief,
                    time,
                    membership,
                    disputes: disputes.get(&estimate.id).cloned().unwrap_or_default(),
                });
        }
    }
    for entity in &mut *entities {
        entity.origins = origins.remove(&entity.entity_id).unwrap_or_default();
    }
    entities.extend(origins.into_iter().map(|(entity_id, origins)| EntitySlice {
        entity_id,
        origins,
        properties: Vec::new(),
    }));
    entities.sort_by_key(|entity| entity.entity_id);
    Ok(())
}
