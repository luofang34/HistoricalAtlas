use super::{PlaceMap, PlaceMapError, register_places};
use crate::{
    input::LedgerInput,
    registration::{RegisteredSheet, RegistrationJobError},
};
use atlas_boundary::{geographic::GeographicBoundary, linear::GeographicLine, sheet::SheetTrace};
use atlas_ledger::model::Estimate;
use std::collections::BTreeMap;

pub(crate) struct RegisteredMap {
    pub sheet: RegisteredSheet,
    pub estimates: Vec<Estimate>,
}

pub(crate) struct RegisteredShapes {
    pub boundaries: Vec<GeographicBoundary>,
    pub lines: Vec<GeographicLine>,
    pub maps: BTreeMap<String, RegisteredMap>,
}

pub(crate) fn register_shapes(input: &LedgerInput) -> Result<RegisteredShapes, PlaceMapError> {
    let mut maps = BTreeMap::new();
    for map in input.place_maps.values() {
        let (sheet, estimates) = register_places(input, map)?;
        maps.insert(map.id.clone(), RegisteredMap { sheet, estimates });
    }
    let mut output = RegisteredShapes {
        maps,
        boundaries: Vec::new(),
        lines: Vec::new(),
    };
    for trace in &input.traces {
        let Some(map) = select_map(input, trace)? else {
            continue;
        };
        let registered = output
            .maps
            .get(&map.id)
            .ok_or_else(|| PlaceMapError::Evidence(format!("missing fitted map {}", map.id)))?;
        let (boundaries, lines) = register_trace(input, trace, map, &registered.sheet)?;
        output.boundaries.extend(boundaries);
        output.lines.extend(lines);
    }
    Ok(output)
}

fn select_map<'a>(
    input: &'a LedgerInput,
    trace: &SheetTrace,
) -> Result<Option<&'a PlaceMap>, PlaceMapError> {
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
    match matching.as_slice() {
        [] if trace.registration_id.is_none() => Ok(None),
        [] => Err(PlaceMapError::Evidence(format!(
            "missing registration {:?}",
            trace.registration_id
        ))),
        [map] => Ok(Some(*map)),
        _ => Err(PlaceMapError::Evidence(format!(
            "{} scan page {:?} has multiple place-map registrations",
            trace.source_id, trace.scan_page
        ))),
    }
}

type Shapes = (Vec<GeographicBoundary>, Vec<GeographicLine>);

fn register_trace(
    input: &LedgerInput,
    trace: &SheetTrace,
    map: &PlaceMap,
    sheet: &RegisteredSheet,
) -> Result<Shapes, PlaceMapError> {
    atlas_boundary::sheet::validate(&input.catalog, &input.data, trace)?;
    if map.source_frame.width != trace.source_frame.width
        || map.source_frame.height != trace.source_frame.height
    {
        return Err(PlaceMapError::Evidence(format!(
            "{} boundary and place symbols use different raster dimensions",
            map.id
        )));
    }
    let polygons = trace
        .units
        .iter()
        .flat_map(|unit| unit.polygons.iter().cloned())
        .collect::<Vec<_>>();
    let (vertices, polygons) =
        atlas_boundary::polygon::densify(&trace.vertices, &polygons, map.maximum_segment_pixels)?;
    let (vertices, lines) =
        atlas_boundary::linear::densify(&vertices, &trace.lines, map.maximum_segment_pixels)?;
    let vertices = vertices
        .iter()
        .map(|p| sheet.transform(*p))
        .collect::<Result<Vec<_>, _>>()
        .map_err(RegistrationJobError::from)?;
    let mut boundaries = Vec::new();
    let mut remaining = polygons.into_iter();
    for unit in &trace.units {
        let mut unit = unit.clone();
        unit.polygons = remaining.by_ref().take(unit.polygons.len()).collect();
        let (unit_vertices, unit) = compact_unit(&vertices, &unit)
            .map_err(|i| PlaceMapError::Evidence(format!("missing unit vertex {i}")))?;
        let boundary = GeographicBoundary {
            unit,
            vertices: unit_vertices,
            datum: map.datum,
            registration_allowance_m: Some(map.uncertainty_m),
            boundary_method: trace.boundary_method,
            method_basis: trace.method_basis.clone(),
            registration: format!("maps/{}.json", map.id),
        };
        boundary.feature(&input.data, None)?;
        boundaries.push(boundary);
    }
    let mut registered_lines = Vec::new();
    for line in lines {
        let (line_vertices, line) = compact_line(&vertices, &line)
            .map_err(|i| PlaceMapError::Evidence(format!("missing line vertex {i}")))?;
        let geographic = GeographicLine {
            line,
            vertices: line_vertices,
            datum: map.datum,
            registration: format!("maps/{}.json", map.id),
        };
        geographic.feature(&input.data, None)?;
        registered_lines.push(geographic);
    }
    Ok((boundaries, registered_lines))
}

pub(crate) fn compact_unit(
    vertices: &[atlas_geo::registration::PlanarPoint],
    unit: &atlas_boundary::sheet::TracedUnit,
) -> Result<
    (
        Vec<atlas_geo::registration::PlanarPoint>,
        atlas_boundary::sheet::TracedUnit,
    ),
    usize,
> {
    let mut remap = BTreeMap::new();
    let mut compact = Vec::new();
    let mut map_ring = |ring: &[usize]| -> Result<Vec<usize>, usize> {
        ring.iter()
            .map(|&idx| {
                if let Some(&mapped) = remap.get(&idx) {
                    return Ok(mapped);
                }
                let point = *vertices.get(idx).ok_or(idx)?;
                let mapped = compact.len();
                compact.push(point);
                remap.insert(idx, mapped);
                Ok(mapped)
            })
            .collect()
    };
    let polygons = unit
        .polygons
        .iter()
        .map(|poly| {
            let exterior = map_ring(&poly.exterior)?;
            let interiors = poly
                .interiors
                .iter()
                .map(|ring| map_ring(ring))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(atlas_boundary::polygon::IndexedPolygon {
                exterior,
                interiors,
            })
        })
        .collect::<Result<Vec<_>, usize>>()?;
    let mut updated = unit.clone();
    updated.polygons = polygons;
    Ok((compact, updated))
}

pub(crate) fn compact_line(
    vertices: &[atlas_geo::registration::PlanarPoint],
    line: &atlas_boundary::linear::TracedLine,
) -> Result<
    (
        Vec<atlas_geo::registration::PlanarPoint>,
        atlas_boundary::linear::TracedLine,
    ),
    usize,
> {
    let mut remap = BTreeMap::new();
    let mut compact = Vec::new();
    let paths = line
        .paths
        .iter()
        .map(|path| {
            path.iter()
                .map(|&idx| {
                    if let Some(&mapped) = remap.get(&idx) {
                        return Ok(mapped);
                    }
                    let point = *vertices.get(idx).ok_or(idx)?;
                    let mapped = compact.len();
                    compact.push(point);
                    remap.insert(idx, mapped);
                    Ok(mapped)
                })
                .collect::<Result<Vec<_>, usize>>()
        })
        .collect::<Result<Vec<_>, usize>>()?;
    let mut updated = line.clone();
    updated.paths = paths;
    Ok((compact, updated))
}

pub(crate) fn register_boundaries(
    input: &LedgerInput,
) -> Result<Vec<GeographicBoundary>, PlaceMapError> {
    Ok(register_shapes(input)?.boundaries)
}

#[cfg(test)]
mod tests;
