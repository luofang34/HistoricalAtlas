use super::*;

pub(super) fn publish_maps(
    output: &mut Artifacts,
    input: &LedgerInput,
    maps: &BTreeMap<String, crate::place_maps::RegisteredMap>,
) -> Result<(), ExportError> {
    let names: Vec<_> = input.place_maps.keys().collect();
    insert(output, "maps/index.json", &names)?;
    for (id, registered) in maps {
        let map = input.place_maps.get(id);
        let registration = &registered.sheet;
        let estimates = &registered.estimates;
        insert(
            output,
            &format!("maps/{id}.json"),
            &serde_json::json!({
                "map": map, "registration": registration,
                "estimates": estimates.iter().map(|e| e.id).collect::<Vec<_>>()
            }),
        )?;
    }
    Ok(())
}

pub(super) fn index<'a>(
    input: &'a LedgerInput,
    entities: &mut BTreeMap<EntityId, IndexedEntity<'a>>,
    boundaries: &'a [GeographicBoundary],
    derived: &'a [atlas_boundary::terrain::DerivedBoundary],
    lines: &'a [atlas_boundary::linear::GeographicLine],
) {
    for trace in &input.traces {
        if let Some(id) = trace
            .units
            .first()
            .map(|unit| unit.record_id)
            .or_else(|| trace.lines.first().map(|line| line.record_id))
            && let Some(record) = input.data.records.iter().find(|r| r.id == id)
            && let Some(index) = entities.get_mut(&record.entity_id)
        {
            index.drawings.push(trace);
        }
    }
    for boundary in boundaries {
        if let Some(record) = input
            .data
            .records
            .iter()
            .find(|r| r.id == boundary.unit.record_id)
            && let Some(index) = entities.get_mut(&record.entity_id)
        {
            index.boundaries.push(boundary);
        }
    }
    for boundary in derived {
        if let Some(record) = input
            .data
            .records
            .iter()
            .find(|r| r.id == boundary.record_id)
            && let Some(index) = entities.get_mut(&record.entity_id)
        {
            index.derived_boundaries.push(boundary);
        }
    }
    for line in lines {
        if let Some(record) = input
            .data
            .records
            .iter()
            .find(|r| r.id == line.line.record_id)
            && let Some(index) = entities.get_mut(&record.entity_id)
        {
            index.lines.push(line);
        }
    }
}

pub(crate) fn features(
    output: &Artifacts,
    data: &atlas_ledger::model::Dataset,
) -> Result<Vec<serde_json::Value>, ExportError> {
    let mut features = Vec::new();
    for (_, bytes) in output
        .iter()
        .filter(|(path, _)| path.starts_with("entities/index"))
    {
        let entries: BTreeMap<String, serde_json::Value> = serde_json::from_slice(bytes)?;
        for entry in entries.values() {
            for key in ["boundaries", "derived_boundaries", "lines"] {
                for shape in entry
                    .get(key)
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let feature = match key {
                        "boundaries" => {
                            serde_json::from_value::<GeographicBoundary>(shape.clone())?
                                .feature(data, None)?
                        }
                        "lines" => {
                            serde_json::from_value::<atlas_boundary::linear::GeographicLine>(
                                shape.clone(),
                            )?
                            .feature(data, None)?
                        }
                        _ => serde_json::from_value::<atlas_boundary::terrain::DerivedBoundary>(
                            shape.clone(),
                        )?
                        .feature(data, None)?,
                    };
                    features.push(feature);
                }
            }
        }
    }
    Ok(features)
}
