use super::*;
use atlas_boundary::terrain::TerrainSeed;
use atlas_core::identity::EntityId;
use atlas_geo::coordinates::Datum;
use atlas_ledger::model::{EstimateValue, Record};
use std::collections::VecDeque;

pub(super) fn validate(
    input: &LedgerInput,
    request: &DerivationRequest,
) -> Result<(), DerivationError> {
    checked(input, request.envelope_record_id, &request.source_id)?;
    if request.id.is_empty()
        || !request
            .id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || request.alignment.trim().is_empty()
        || request.terrain_tiles.is_empty()
        || !input
            .catalog
            .sources
            .iter()
            .any(|s| s.id == request.terrain_source_id)
        || !input
            .catalog
            .sources
            .iter()
            .any(|s| s.id == request.source_id)
    {
        return Err(DerivationError::Evidence(
            "recipe needs an identifier, alignment, and catalogued sources".into(),
        ));
    }
    let mut tiles = BTreeSet::new();
    for tile in &request.terrain_tiles {
        if !tile.url.starts_with("https://")
            || !tiles.insert(&tile.url)
            || tile.sha256.len() != 64
            || !tile
                .sha256
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(DerivationError::Evidence(
                "invalid or duplicate terrain tile fingerprint".into(),
            ));
        }
    }
    let mut excluded = BTreeSet::new();
    for anchor in &request.excluded {
        checked(input, anchor.record_id, &request.source_id)?;
        if anchor.reason.trim().is_empty() || !excluded.insert(anchor.record_id) {
            return Err(DerivationError::Evidence(
                "excluded anchors need distinct records and a reason".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn seeds(
    input: &LedgerInput,
    request: &DerivationRequest,
    datum: Datum,
) -> Result<(Vec<TerrainSeed>, Vec<BTreeSet<FactId>>), DerivationError> {
    let mut seeds = Vec::new();
    let mut bases = Vec::new();
    let mut subjects = BTreeSet::new();
    for (index, unit) in request.units.iter().enumerate() {
        let record = checked(input, unit.record_id, &request.source_id)?;
        if record.observation.rank.is_none()
            || !subjects.insert(record.entity_id)
            || unit.anchors.is_empty()
        {
            return Err(DerivationError::Evidence(
                "units need distinct administrative subjects and seed evidence".into(),
            ));
        }
        let mut basis = BTreeSet::from([record.id]);
        for anchor in &unit.anchors {
            let (entity, point, anchor_datum, proof) = location(input, request, anchor)?;
            if anchor_datum != datum {
                return Err(DerivationError::Evidence(
                    "seed and envelope datums differ".into(),
                ));
            }
            basis.extend(lineage(
                input,
                entity,
                record.entity_id,
                &request.source_id,
            )?);
            basis.extend(proof);
            seeds.push(TerrainSeed { unit: index, point });
        }
        bases.push(basis);
    }
    Ok((seeds, bases))
}

fn location(
    input: &LedgerInput,
    request: &DerivationRequest,
    anchor: &AnchorReference,
) -> Result<(EntityId, atlas_geo::coordinates::LonLat, Datum, Vec<FactId>), DerivationError> {
    match anchor {
        AnchorReference::Record { record_id } => {
            let record = checked(input, *record_id, &request.source_id)?;
            if request.excluded.iter().any(|a| a.record_id == *record_id) {
                return Err(DerivationError::Evidence(
                    "excluded record is also a seed".into(),
                ));
            }
            let coordinate = record.observation.coordinate.as_ref().ok_or_else(|| {
                DerivationError::Evidence(format!("record {record_id:?} has no coordinate"))
            })?;
            Ok((
                record.entity_id,
                coordinate.point,
                coordinate.datum,
                vec![record.id],
            ))
        }
        AnchorReference::MapLocation { estimate_id } => {
            let estimate = input
                .data
                .estimates
                .iter()
                .find(|e| e.id == *estimate_id)
                .ok_or_else(|| {
                    DerivationError::Evidence(format!("missing location estimate {estimate_id:?}"))
                })?;
            let EstimateValue::MapLocation {
                record_id,
                point,
                datum,
                ..
            } = estimate.value
            else {
                return Err(DerivationError::Evidence(
                    "seed estimate must be a registered map location".into(),
                ));
            };
            checked(input, record_id, &request.source_id)?;
            Ok((estimate.entity_id, point, datum, estimate.basis.clone()))
        }
    }
}

fn lineage(
    input: &LedgerInput,
    child: EntityId,
    parent: EntityId,
    source: &str,
) -> Result<BTreeSet<FactId>, DerivationError> {
    let mut queue = VecDeque::from([(child, BTreeSet::new())]);
    let mut visited = BTreeSet::new();
    while let Some((current, path)) = queue.pop_front() {
        if current == parent {
            return Ok(path);
        }
        if !visited.insert(current) {
            continue;
        }
        let mut records: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| r.checked && r.source_id == source && r.entity_id == current)
            .collect();
        records.sort_by_key(|record| record.id);
        for record in records {
            if let Some(next) = record.observation.parent_id {
                let mut proof = path.clone();
                proof.insert(record.id);
                queue.push_back((next, proof));
            }
        }
    }
    Err(DerivationError::Evidence(format!(
        "seed subject {child:?} has no source hierarchy path to {parent:?}"
    )))
}

fn checked<'a>(
    input: &'a LedgerInput,
    id: FactId,
    source: &str,
) -> Result<&'a Record, DerivationError> {
    input
        .data
        .records
        .iter()
        .find(|r| r.id == id && r.checked && r.source_id == source)
        .ok_or_else(|| DerivationError::Evidence(format!("missing checked {source} record {id:?}")))
}
