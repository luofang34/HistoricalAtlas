use super::{PlaceControl, PlaceMap};
use crate::{input::LedgerInput, registration::RegistrationJobError};
use atlas_core::identity::FactId;
use atlas_ledger::model::Record;
use std::collections::BTreeSet;
use thiserror::Error;

/// Invalid map evidence, registration, or derived location.
#[derive(Debug, Error)]
pub enum PlaceMapError {
    /// A matching sheet contains invalid source geometry or evidence.
    #[error("place map boundary sheet: {0}")]
    Sheet(#[from] atlas_boundary::sheet::SheetError),
    /// Registered boundary coordinates or evidence are invalid.
    #[error("place map boundary: {0}")]
    Boundary(#[from] atlas_boundary::geographic::GeographicError),
    /// Missing or incompatible map or coordinate evidence.
    #[error("place map evidence: {0}")]
    Evidence(String),
    /// Registration failed.
    #[error("place map registration: {0}")]
    Registration(#[from] RegistrationJobError),
    /// Derived coordinates lie outside the angular domain.
    #[error("place map location: {0}")]
    Coordinate(#[from] atlas_geo::coordinates::CoordinateError),
    /// Derived identity key is invalid.
    #[error("place map identity: {0}")]
    Identity(#[from] atlas_core::identity::InvalidKey),
}

pub(super) fn record(input: &LedgerInput, id: FactId) -> Result<&Record, PlaceMapError> {
    input
        .data
        .records
        .iter()
        .find(|r| r.id == id && r.checked)
        .ok_or_else(|| PlaceMapError::Evidence(format!("missing checked record {id:?}")))
}

pub(super) fn validate(input: &LedgerInput, map: &PlaceMap) -> Result<(), PlaceMapError> {
    if map.id.is_empty()
        || !map
            .id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || map.places.is_empty()
        || map.check_points.is_empty()
        || !map.uncertainty_m.is_finite()
        || map.uncertainty_m <= 0.0
        || map.uncertainty_basis.trim().is_empty()
    {
        return Err(PlaceMapError::Evidence(
            "map requires an ASCII id, places, holdouts, and a justified positive uncertainty"
                .into(),
        ));
    }
    let mut subjects = BTreeSet::new();
    let mut body = None;
    for place in &map.places {
        let record = record(input, place.record_id)?;
        let entity = input
            .data
            .entities
            .iter()
            .find(|entity| entity.id == record.entity_id)
            .ok_or_else(|| {
                PlaceMapError::Evidence(format!("missing subject {:?}", record.entity_id))
            })?;
        map.datum.validate_body(entity.body)?;
        if body.is_some_and(|body| body != entity.body) {
            return Err(PlaceMapError::Evidence(
                "map requires a unique subject body".into(),
            ));
        }
        body = Some(entity.body);
        if record.source_id != map.source_id
            || record.scan_page != Some(map.scan_page)
            || !subjects.insert(record.entity_id)
        {
            return Err(PlaceMapError::Evidence(format!(
                "{} requires a unique subject and the mapped sheet",
                record.key
            )));
        }
    }
    let mut used = BTreeSet::new();
    for control in map.controls.iter().chain(&map.check_points) {
        if !used.insert(control.map_record_id) {
            return Err(PlaceMapError::Evidence(
                "fitting and checking controls must be independent".into(),
            ));
        }
        validate_control(input, map, control)?;
    }
    Ok(())
}

fn validate_control(
    input: &LedgerInput,
    map: &PlaceMap,
    control: &PlaceControl,
) -> Result<(), PlaceMapError> {
    let mapped = record(input, control.map_record_id)?;
    let target = record(input, control.coordinate_record_id)?;
    if !map.places.iter().any(|p| p.record_id == mapped.id)
        || mapped.entity_id != target.entity_id
        || target.id == mapped.id
        || target
            .observation
            .coordinate
            .as_ref()
            .is_none_or(|c| c.datum != map.datum)
    {
        return Err(PlaceMapError::Evidence(format!(
            "{} control requires a measured symbol and same-entity coordinate in the declared datum",
            mapped.key
        )));
    }
    Ok(())
}
