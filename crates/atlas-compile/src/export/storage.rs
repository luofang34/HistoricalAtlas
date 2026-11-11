use super::*;
use atlas_boundary::publication::{GeometryPart, GeometryReference, fingerprint};

pub(super) fn detach(
    output: &mut Artifacts,
    entities: &mut BTreeMap<EntityId, IndexedEntity<'_>>,
    data: &atlas_ledger::model::Dataset,
) -> Result<(), ExportError> {
    let records: BTreeMap<_, _> = data.records.iter().map(|r| (r.id, r.entity_id)).collect();
    for (id, entry) in entities {
        if entry.drawings.is_empty()
            && entry.boundaries.is_empty()
            && entry.derived_boundaries.is_empty()
            && entry.lines.is_empty()
        {
            continue;
        }
        let subjects: BTreeSet<_> = entry
            .drawings
            .iter()
            .flat_map(|trace| {
                trace
                    .units
                    .iter()
                    .map(|u| u.record_id)
                    .chain(trace.places.iter().map(|p| p.record_id))
                    .chain(trace.lines.iter().map(|l| l.record_id))
            })
            .filter_map(|id| records.get(&id).copied())
            .collect();
        let bounds = bounds(entry);
        let bytes = serde_json::to_vec(
            &serde_json::json!({"drawings":entry.drawings,"boundaries":entry.boundaries,"derived_boundaries":entry.derived_boundaries,"lines":entry.lines}),
        )?;
        let mut parts = Vec::new();
        for (index, bytes) in bytes.chunks(2_000_000).enumerate() {
            let path = format!("geometry/{}/{index:03}.part", id.0);
            parts.push(GeometryPart {
                path: path.clone(),
                bytes: bytes.len(),
                sha256: fingerprint(bytes),
            });
            insert_bytes(output, &path, bytes.to_vec())?;
        }
        let reference = GeometryReference {
            parts,
            bounds,
            subjects: subjects.into_iter().collect(),
        };
        reference.validate()?;
        entry.geometry = Some(reference);
        entry.drawings.clear();
        entry.boundaries.clear();
        entry.derived_boundaries.clear();
        entry.lines.clear();
    }
    Ok(())
}

fn bounds(entry: &IndexedEntity<'_>) -> Option<[f64; 4]> {
    let mut points = entry
        .boundaries
        .iter()
        .flat_map(|b| {
            b.unit
                .polygons
                .iter()
                .flat_map(|p| &p.exterior)
                .filter_map(|i| b.vertices.get(*i))
        })
        .chain(entry.derived_boundaries.iter().flat_map(|b| {
            b.region
                .polygons
                .iter()
                .flat_map(|p| &p.exterior)
                .filter_map(|i| b.region.vertices.get(*i))
        }))
        .chain(entry.lines.iter().flat_map(|l| {
            l.line
                .paths
                .iter()
                .flatten()
                .filter_map(|i| l.vertices.get(*i))
        }));
    let point = points.next()?;
    Some(
        points.fold([point.x, point.y, point.x, point.y], |[w, s, e, n], p| {
            [w.min(p.x), s.min(p.y), e.max(p.x), n.max(p.y)]
        }),
    )
}

pub(crate) fn resolve(
    output: &Artifacts,
    entry: &serde_json::Value,
) -> Result<serde_json::Value, ExportError> {
    let Some(value) = entry.get("geometry") else {
        return Ok(entry.clone());
    };
    let reference: GeometryReference = serde_json::from_value(value.clone())?;
    reference.validate()?;
    let parts = reference
        .parts
        .iter()
        .map(|part| {
            output
                .get(Path::new(&part.path))
                .cloned()
                .ok_or_else(|| atlas_boundary::publication::GeometryError::Part(part.path.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bundle = reference.decode(&parts)?;
    Ok(serde_json::to_value(bundle)?)
}
