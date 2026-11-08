//! Cross-reference validation before publishing an evidence ledger.

use std::collections::BTreeMap;

use atlas_core::identity::{EntityId, FactId};
pub(crate) mod error;
mod estimates;
mod hierarchy;
mod temporal;
pub use error::{ValidationCause, ValidationError};
use error::{caused, fail};
use estimates::validate_estimates;

use crate::model::{Dataset, Entity, Record, Source, SourceCatalog};

/// Validate source fingerprints, identifiers, references, and interpreted values.
pub fn validate(catalog: &SourceCatalog, data: &Dataset) -> Result<(), ValidationError> {
    let sources = validate_sources(catalog)?;
    let mut entities = BTreeMap::new();
    for entity in &data.entities {
        let expected = EntityId::from_key(&entity.key).map_err(|e| caused(&entity.key, e))?;
        if entity.id != expected || entities.insert(entity.id, entity).is_some() {
            return Err(fail(
                &entity.key,
                "duplicate or noncanonical entity identity",
            ));
        }
    }
    let mut facts = BTreeMap::new();
    for record in &data.records {
        validate_record(record, &entities, &sources)?;
        let expected = FactId::from_key(&record.key).map_err(|e| caused(&record.key, e))?;
        if record.id != expected || facts.insert(record.id, record).is_some() {
            return Err(fail(&record.key, "duplicate or noncanonical fact identity"));
        }
    }
    validate_estimates(data, &entities, &facts)?;
    hierarchy::validate_hierarchy(data)?;
    Ok(())
}

fn validate_sources(catalog: &SourceCatalog) -> Result<BTreeMap<&str, &Source>, ValidationError> {
    let mut sources = BTreeMap::new();
    for source in &catalog.sources {
        if source.id.trim().is_empty() || source.title.trim().is_empty() || source.scan_pages == 0 {
            return Err(fail(
                &source.id,
                "source requires an id, title, and nonzero scan page count",
            ));
        }
        if source.scan_sha256.len() != 64
            || !source.scan_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(fail(
                &source.id,
                "source scan fingerprint must be a SHA-256 digest",
            ));
        }
        if sources.insert(source.id.as_str(), source).is_some() {
            return Err(fail(&source.id, "duplicate source id"));
        }
    }
    Ok(sources)
}

fn validate_record(
    record: &Record,
    entities: &BTreeMap<EntityId, &Entity>,
    sources: &BTreeMap<&str, &Source>,
) -> Result<(), ValidationError> {
    let entity = entities
        .get(&record.entity_id)
        .ok_or_else(|| fail(&record.key, "unknown entity"))?;
    let source = sources
        .get(record.source_id.as_str())
        .ok_or_else(|| fail(&record.key, "unknown source"))?;
    if record.scan_page == 0
        || record.scan_page > source.scan_pages
        || record.page_or_sheet.trim().is_empty()
        || record.locator.trim().is_empty()
        || record.observation.name.trim().is_empty()
    {
        return Err(fail(&record.key, "invalid page, locator, or name"));
    }
    for target in record
        .observation
        .parent_id
        .iter()
        .chain(&record.observation.seat_id)
    {
        temporal::validate_relation(&record.key, entity.id, *target, entities)?;
    }
    if let Some(coordinate) = &record.observation.coordinate {
        coordinate
            .validate(entity.body)
            .map_err(|e| caused(&record.key, e))?;
        if coordinate.longitude_text.trim().is_empty() || coordinate.latitude_text.trim().is_empty()
        {
            return Err(fail(&record.key, "coordinate requires original notation"));
        }
    }
    for former in &record.observation.former_names {
        if former.name.trim().is_empty() {
            return Err(fail(&record.key, "empty former name"));
        }
        if let Some(span) = &former.validity {
            span.validate().map_err(|e| caused(&record.key, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
