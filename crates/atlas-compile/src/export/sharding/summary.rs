use std::collections::{BTreeMap, BTreeSet};

use atlas_boundary::publication::{CatalogItem, ShardPlateRef, ShardTemporalSummary};
use atlas_core::{
    assertion::TemporalValue,
    identity::{EntityId, FactId},
    origin::OriginTime,
    time::TemporalSpan,
};
use atlas_geo::body::BodyId;
use atlas_ledger::model::EstimateValue;

use super::super::IndexedEntity;
use crate::input::LedgerInput;

pub(crate) type GeometrySpans = BTreeMap<EntityId, Vec<Option<TemporalSpan>>>;

pub(crate) const fn body_slug(body: BodyId) -> &'static str {
    match body {
        BodyId::Earth => "earth",
        BodyId::Moon => "moon",
        BodyId::Mars => "mars",
    }
}

pub(crate) fn collect_metadata(
    entities: &BTreeMap<EntityId, IndexedEntity<'_>>,
    input: &LedgerInput,
    derived: &[atlas_boundary::terrain::DerivedBoundary],
) -> (GeometrySpans, BTreeSet<EntityId>) {
    let record_owners: BTreeMap<FactId, EntityId> = input
        .data
        .records
        .iter()
        .filter(|r| r.checked)
        .map(|r| (r.id, r.entity_id))
        .collect();
    let mut cross_cited = BTreeSet::new();
    for estimate in &input.data.estimates {
        for basis_id in &estimate.basis {
            if let Some(&owner) = record_owners.get(basis_id)
                && owner != estimate.entity_id
            {
                cross_cited.insert(owner);
            }
        }
    }
    for boundary in derived {
        let subject = record_owners.get(&boundary.record_id).copied();
        for basis_id in &boundary.basis {
            if let Some(&owner) = record_owners.get(basis_id)
                && Some(owner) != subject
            {
                cross_cited.insert(owner);
            }
        }
    }
    for trace in &input.traces {
        for fact_id in trace
            .units
            .iter()
            .map(|u| u.record_id)
            .chain(trace.places.iter().map(|p| p.record_id))
            .chain(trace.lines.iter().map(|l| l.record_id))
        {
            if let Some(&owner) = record_owners.get(&fact_id) {
                cross_cited.insert(owner);
            }
        }
    }
    let mut spans = GeometrySpans::new();
    for (id, entry) in entities {
        let list = spans.entry(*id).or_default();
        list.extend(entry.boundaries.iter().map(|b| b.unit.validity.clone()));
        list.extend(entry.derived_boundaries.iter().map(|b| b.validity.clone()));
        list.extend(entry.lines.iter().map(|l| l.line.validity.clone()));
    }
    (spans, cross_cited)
}

pub(super) fn entity_bounds(entry: &IndexedEntity<'_>) -> Option<[f64; 4]> {
    let mut envelope: Option<[f64; 4]> = entry.geometry.as_ref().and_then(|g| g.bounds);
    let mut push = |lon: f64, lat: f64| {
        envelope = Some(match envelope {
            Some([w, s, e, n]) => [w.min(lon), s.min(lat), e.max(lon), n.max(lat)],
            None => [lon, lat, lon, lat],
        });
    };
    for record in &entry.records {
        if let Some(coord) = &record.observation.coordinate {
            push(coord.point.longitude, coord.point.latitude);
        }
    }
    for estimate in &entry.estimates {
        match &estimate.value {
            EstimateValue::Location { point, .. } | EstimateValue::MapLocation { point, .. } => {
                push(point.longitude, point.latitude);
            }
            _ => {}
        }
    }
    envelope
}

pub(super) fn merge_bounds(a: Option<[f64; 4]>, b: Option<[f64; 4]>) -> Option<[f64; 4]> {
    match (a, b) {
        (Some([w1, s1, e1, n1]), Some([w2, s2, e2, n2])) => {
            Some([w1.min(w2), s1.min(s2), e1.max(e2), n1.max(n2)])
        }
        (Some(v), None) | (None, Some(v)) => Some(v),
        (None, None) => None,
    }
}

pub(super) fn entity_temporal(
    entry: &IndexedEntity<'_>,
    geometry_spans: Option<&Vec<Option<TemporalSpan>>>,
) -> ShardTemporalSummary {
    let mut out = ShardTemporalSummary::default();
    let mut plates = BTreeSet::new();
    let apply_span = |out: &mut ShardTemporalSummary, span: &TemporalSpan| {
        out.has_temporal = true;
        if let Some(start) = &span.start {
            out.earliest = Some(
                out.earliest
                    .map_or(start.earliest, |cur| cur.min(start.earliest)),
            );
        } else {
            out.open_start = true;
        }
        if let Some(end) = &span.end {
            out.latest = Some(out.latest.map_or(end.latest, |cur| cur.max(end.latest)));
        } else {
            out.open_end = true;
        }
    };
    for estimate in &entry.estimates {
        match &estimate.value {
            EstimateValue::Temporal { assertion } => apply_span(&mut out, &assertion.validity),
            EstimateValue::Origin { time } => {
                out.has_temporal = true;
                out.open_end = true;
                match time {
                    OriginTime::Estimated { instant } => {
                        out.earliest = Some(
                            out.earliest
                                .map_or(instant.earliest, |cur| cur.min(instant.earliest)),
                        );
                    }
                    OriginTime::NoLaterThan { .. } | OriginTime::Approximate { .. } => {
                        out.open_start = true;
                    }
                }
            }
            EstimateValue::Plate {
                model_id, plate_id, ..
            } => {
                plates.insert(ShardPlateRef {
                    model_id: model_id.clone(),
                    plate_id: *plate_id,
                });
            }
            _ => {}
        }
    }
    for span in geometry_spans.into_iter().flatten() {
        if let Some(span) = span {
            apply_span(&mut out, span);
        } else {
            out.has_temporal = true;
            out.open_start = true;
            out.open_end = true;
        }
    }
    out.plates = plates.into_iter().collect();
    out
}

pub(super) fn merge_temporal(
    mut acc: ShardTemporalSummary,
    other: &ShardTemporalSummary,
) -> ShardTemporalSummary {
    acc.has_temporal |= other.has_temporal;
    acc.open_start |= other.open_start;
    acc.open_end |= other.open_end;
    acc.earliest = match (acc.earliest, other.earliest) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, b) => b,
    };
    acc.latest = match (acc.latest, other.latest) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, b) => b,
    };
    for plate in &other.plates {
        if !acc.plates.contains(plate) {
            acc.plates.push(plate.clone());
        }
    }
    acc.plates.sort();
    acc
}

pub(super) fn build_catalog_item(entry: &IndexedEntity<'_>) -> CatalogItem {
    let label = entry
        .records
        .iter()
        .find(|r| !r.observation.name.is_empty())
        .map_or_else(|| entry.entity.key.clone(), |r| r.observation.name.clone());
    let mut terms = BTreeSet::new();
    let mut ranks = Vec::new();
    let mut parents = BTreeSet::new();
    for record in &entry.records {
        if !record.observation.name.is_empty() && record.observation.name != label {
            terms.insert(record.observation.name.clone());
        }
        if let Some(pinyin) = &record.observation.pinyin
            && !pinyin.is_empty()
        {
            terms.insert(pinyin.clone());
        }
        for former in &record.observation.former_names {
            if !former.name.is_empty() {
                terms.insert(former.name.clone());
            }
        }
        if let Some(rank) = record.observation.rank
            && !ranks.contains(&rank)
        {
            ranks.push(rank);
        }
        if let Some(parent) = record.observation.parent_id {
            parents.insert(parent);
        }
    }
    for estimate in &entry.estimates {
        if let EstimateValue::Temporal { assertion } = &estimate.value {
            match &assertion.value {
                TemporalValue::Hierarchy(h) => {
                    if !ranks.contains(&h.rank) {
                        ranks.push(h.rank);
                    }
                }
                TemporalValue::Name(n) if !n.text.is_empty() && n.text != label => {
                    terms.insert(n.text.clone());
                }
                _ => {}
            }
        }
    }
    CatalogItem {
        id: entry.entity.id,
        key: entry.entity.key.clone(),
        body: entry.entity.body,
        kind: entry.entity.kind,
        shard: String::new(),
        label,
        terms: terms.into_iter().collect(),
        ranks,
        parents: parents.into_iter().collect(),
    }
}

pub(super) fn assign_dependencies(
    entities: &BTreeMap<EntityId, IndexedEntity<'_>>,
    catalog_items: &[CatalogItem],
    shards: &mut [atlas_boundary::publication::ShardSummary],
) {
    let entity_shard: BTreeMap<EntityId, &str> = catalog_items
        .iter()
        .map(|item| (item.id, item.shard.as_str()))
        .collect();
    let record_owners: BTreeMap<FactId, EntityId> = entities
        .values()
        .flat_map(|entry| entry.records.iter().map(|r| (r.id, r.entity_id)))
        .collect();
    let mut deps: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (id, entry) in entities {
        let Some(&source_shard) = entity_shard.get(id) else {
            continue;
        };
        for estimate in &entry.estimates {
            for basis_id in &estimate.basis {
                if let Some(owner) = record_owners.get(basis_id)
                    && let Some(&target_shard) = entity_shard.get(owner)
                    && target_shard != source_shard
                {
                    deps.entry(source_shard)
                        .or_default()
                        .insert(target_shard.to_owned());
                }
            }
        }
    }
    for shard in shards {
        if let Some(required) = deps.remove(shard.id.as_str()) {
            shard.depends_on = required.into_iter().collect();
        }
    }
}

fn reachable(adj: &BTreeMap<EntityId, BTreeSet<EntityId>>, start: EntityId) -> BTreeSet<EntityId> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![start];
    while let Some(node) = stack.pop() {
        if seen.insert(node)
            && let Some(neighbors) = adj.get(&node)
        {
            stack.extend(neighbors.iter().copied());
        }
    }
    seen
}

pub(super) fn citation_ranks(
    entities: &BTreeMap<EntityId, IndexedEntity<'_>>,
) -> (BTreeMap<EntityId, EntityId>, BTreeMap<EntityId, usize>) {
    let record_owners: BTreeMap<FactId, EntityId> = entities
        .values()
        .flat_map(|entry| entry.records.iter().map(|r| (r.id, r.entity_id)))
        .collect();
    let mut adj: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
    for (&id, entry) in entities {
        let targets: BTreeSet<EntityId> = entry
            .estimates
            .iter()
            .flat_map(|est| &est.basis)
            .filter_map(|fact| record_owners.get(fact).copied())
            .filter(|&owner| owner != id)
            .collect();
        if !targets.is_empty() {
            adj.insert(id, targets);
        }
    }
    let reach: BTreeMap<EntityId, BTreeSet<EntityId>> =
        adj.keys().map(|&id| (id, reachable(&adj, id))).collect();
    let mut reps = BTreeMap::new();
    for (&id, r_id) in &reach {
        let rep = r_id
            .iter()
            .copied()
            .filter(|other| *other == id || reach.get(other).is_some_and(|ro| ro.contains(&id)))
            .min()
            .unwrap_or(id);
        reps.insert(id, rep);
    }
    let mut dag: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
    for (&u, targets) in &adj {
        let ru = reps.get(&u).copied().unwrap_or(u);
        for &v in targets {
            let rv = reps.get(&v).copied().unwrap_or(v);
            if ru != rv {
                dag.entry(ru).or_default().insert(rv);
            }
        }
    }
    let mut ranks: BTreeMap<EntityId, usize> = BTreeMap::new();
    for _ in 0..=dag.len() {
        let mut changed = false;
        for (&u, targets) in &dag {
            let next = targets
                .iter()
                .map(|v| ranks.get(v).copied().unwrap_or(0).saturating_add(1))
                .max()
                .unwrap_or(0);
            if ranks.get(&u).copied().unwrap_or(0) < next {
                ranks.insert(u, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (reps, ranks)
}
