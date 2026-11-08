use super::*;
use std::collections::BTreeSet;

pub(super) fn validate(
    record: &Record,
    subject: &Entity,
    entities: &BTreeMap<EntityId, &Entity>,
) -> Result<(), ValidationError> {
    let Some(territory) = &record.observation.territory else {
        return Ok(());
    };
    if !territory.area_square_km.is_finite() || territory.area_square_km <= 0.0 {
        return Err(fail(
            &record.key,
            "territory area must be finite and positive",
        ));
    }
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for neighbor in &territory.neighbors {
        if neighbor.name.trim().is_empty() || !names.insert(&neighbor.name) {
            return Err(fail(&record.key, "territory neighbors need distinct names"));
        }
        if let Some(id) = neighbor.entity_id {
            temporal::validate_relation(&record.key, subject.id, id, entities)?;
            if !ids.insert(id) {
                return Err(fail(&record.key, "duplicate territory neighbor identity"));
            }
        }
    }
    Ok(())
}
