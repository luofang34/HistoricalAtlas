use super::{PlaceControl, PlaceMap, PlaceMapError, evidence};
use crate::{
    input::LedgerInput,
    registration::{self, ReferencedControl, RegisteredSheet, RegistrationJob, TargetFrame},
};
use atlas_core::identity::FactId;
use atlas_geo::{
    coordinates::LonLat,
    registration::{ControlPoint, PlanarPoint},
};
use atlas_ledger::model::{Estimate, EstimateValue};
use std::collections::BTreeSet;

pub(super) fn register(
    input: &LedgerInput,
    map: &PlaceMap,
) -> Result<(RegisteredSheet, Vec<Estimate>), PlaceMapError> {
    let controls = map
        .controls
        .iter()
        .map(|c| control(input, map, c))
        .collect::<Result<_, _>>()?;
    let check_points = map
        .check_points
        .iter()
        .map(|c| control(input, map, c))
        .collect::<Result<_, _>>()?;
    let sheet = registration::register(
        input,
        RegistrationJob {
            source_id: map.source_id.clone(),
            scan_page: Some(map.scan_page),
            source_frame: map.source_frame,
            target_frame: TargetFrame {
                crs: format!("ATLAS:source-angular:{:?}", map.datum),
                units: "degrees".into(),
            },
            method: map.method,
            controls,
            check_points,
            vertices: map.places.iter().map(|p| p.point).collect(),
        },
    )?;
    let estimates = estimates(input, map, &sheet)?;
    Ok((sheet, estimates))
}

fn control(
    input: &LedgerInput,
    map: &PlaceMap,
    control: &PlaceControl,
) -> Result<ReferencedControl, PlaceMapError> {
    let mapped = evidence::record(input, control.map_record_id)?;
    let target = evidence::record(input, control.coordinate_record_id)?;
    let source = map
        .places
        .iter()
        .find(|p| p.record_id == mapped.id)
        .ok_or_else(|| {
            PlaceMapError::Evidence(format!("missing measured symbol {:?}", mapped.id))
        })?;
    let coordinate =
        target.observation.coordinate.as_ref().ok_or_else(|| {
            PlaceMapError::Evidence(format!("missing coordinate {:?}", target.id))
        })?;
    Ok(ReferencedControl {
        basis: vec![mapped.id, target.id],
        landmark: mapped.observation.name.clone(),
        reference: format!(
            "{}; record {}; printed page {}; scan page {:?}; {} {}; resolution {} arcminutes; datum {:?}",
            target.source_id,
            target.id.0,
            target.page_or_sheet.as_deref().unwrap_or("unnumbered"),
            target.scan_page,
            coordinate.longitude_text,
            coordinate.latitude_text,
            coordinate.resolution_arcminutes,
            coordinate.datum
        ),
        point: ControlPoint {
            source: source.point,
            target: PlanarPoint {
                x: coordinate.point.longitude,
                y: coordinate.point.latitude,
            },
        },
    })
}

fn estimates(
    input: &LedgerInput,
    map: &PlaceMap,
    sheet: &RegisteredSheet,
) -> Result<Vec<Estimate>, PlaceMapError> {
    let controls: BTreeSet<_> = map
        .controls
        .iter()
        .chain(&map.check_points)
        .flat_map(|c| [c.map_record_id, c.coordinate_record_id])
        .collect();
    map.places
        .iter()
        .zip(&sheet.vertices)
        .map(|(place, vertex)| {
            let record = evidence::record(input, place.record_id)?;
            let key = format!("{}/registered-location", record.key);
            let point = LonLat {
                longitude: vertex.x,
                latitude: vertex.y,
            };
            point.validate()?;
            let mut basis = controls.clone();
            basis.insert(record.id);
            Ok(Estimate {
                id: FactId::from_key(&key)?,
                key,
                entity_id: record.entity_id,
                basis: basis.into_iter().collect(),
                brief: format!(
                    "Registered map position from {}. Registration: maps/{}.json. {}",
                    record.page_or_sheet.as_deref().unwrap_or("the cited sheet"),
                    map.id,
                    map.uncertainty_basis
                ),
                value: EstimateValue::MapLocation {
                    record_id: record.id,
                    point,
                    datum: map.datum,
                    uncertainty_m: map.uncertainty_m,
                    registration: format!("maps/{}.json", map.id),
                },
            })
        })
        .collect()
}
