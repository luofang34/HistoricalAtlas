use super::{PlaceMapError, register_places};
use crate::{input::LedgerInput, registration::RegistrationJobError};
use atlas_boundary::geographic::GeographicBoundary;

pub(crate) fn register_boundaries(
    input: &LedgerInput,
) -> Result<Vec<GeographicBoundary>, PlaceMapError> {
    let mut output = Vec::new();
    for trace in &input.traces {
        let matching: Vec<_> = input
            .place_maps
            .values()
            .filter(|map| {
                map.source_id == trace.source_id
                    && map.scan_page == trace.scan_page
                    && trace
                        .registration_id
                        .as_ref()
                        .is_none_or(|id| *id == map.id)
            })
            .collect();
        let map = match matching.as_slice() {
            [] if trace.registration_id.is_none() => continue,
            [] => {
                return Err(PlaceMapError::Evidence(format!(
                    "missing registration {:?}",
                    trace.registration_id
                )));
            }
            [map] => *map,
            _ => {
                return Err(PlaceMapError::Evidence(format!(
                    "{} scan page {} has multiple place-map registrations",
                    trace.source_id, trace.scan_page
                )));
            }
        };
        atlas_boundary::sheet::validate(&input.catalog, &input.data, trace)?;
        if map.source_frame.width != trace.source_frame.width
            || map.source_frame.height != trace.source_frame.height
        {
            return Err(PlaceMapError::Evidence(format!(
                "{} boundary and place symbols use different raster dimensions",
                map.id
            )));
        }
        let (sheet, _) = register_places(input, map)?;
        let vertices = trace
            .vertices
            .iter()
            .map(|p| sheet.transform(*p))
            .collect::<Result<Vec<_>, _>>()
            .map_err(RegistrationJobError::from)?;
        for unit in &trace.units {
            let boundary = GeographicBoundary {
                unit: unit.clone(),
                vertices: vertices.clone(),
                datum: map.datum,
                registration_allowance_m: Some(map.uncertainty_m),
                boundary_method: trace.boundary_method,
                method_basis: trace.method_basis.clone(),
                registration: format!("maps/{}.json", map.id),
            };
            boundary.feature(&input.data, None)?;
            output.push(boundary);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
