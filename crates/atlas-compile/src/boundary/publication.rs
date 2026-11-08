use super::{BoundaryError, BoundaryMethod, SheetTrace, TracedUnit, evidence};
use crate::{
    export::{
        Artifacts,
        chunks::{ChunkKind, insert_chunks},
        insert,
    },
    input::LedgerInput,
    registration::RegisteredSheet,
};
use atlas_boundary::polygon::resolve;
use atlas_geo::body::BodyId;
use serde_json::{Value, json};

pub(super) fn write_trace(
    artifacts: &mut Artifacts,
    input: &LedgerInput,
    trace: &SheetTrace,
) -> Result<(), BoundaryError> {
    let vertices = insert_chunks(
        artifacts,
        "boundary/trace/vertices",
        &trace.vertices,
        ChunkKind::Array,
    )?;
    let units = insert_chunks(
        artifacts,
        "boundary/trace/units",
        &units(input, &trace.units)?,
        ChunkKind::Array,
    )?;
    insert(
        artifacts,
        "boundary-trace.json",
        &json!({
            "version": 1, "geometry_kind": "map_trace", "source_id": trace.source_id,
            "scan_page": trace.scan_page, "scan_sha256": evidence::fingerprint(input, trace)?,
            "source_frame": trace.source_frame, "boundary_method": trace.boundary_method,
            "places": trace.places,
            "method_basis": trace.method_basis, "vertices": vertices, "units": units,
            "vertex_count": trace.vertices.len(), "unit_count": trace.units.len(),
        }),
    )?;
    Ok(())
}

fn units(input: &LedgerInput, units: &[TracedUnit]) -> Result<Vec<Value>, BoundaryError> {
    units
        .iter()
        .map(|unit| {
            let record = evidence::record(input, unit.record_id)?;
            let entity = input
                .data
                .entities
                .iter()
                .find(|entity| entity.id == record.entity_id)
                .ok_or_else(|| {
                    BoundaryError::Evidence(format!("missing entity {:?}", record.entity_id))
                })?;
            Ok(
                json!({"entity_id": record.entity_id, "record_id": record.id,
            "body": entity.body, "rank": record.observation.rank,
            "validity": null, "polygons": unit.polygons}),
            )
        })
        .collect()
}

pub(super) fn write_registered(
    artifacts: &mut Artifacts,
    input: &LedgerInput,
    trace: &SheetTrace,
    sheet: &RegisteredSheet,
    boundaries: &[TracedUnit],
    spacing: f64,
) -> Result<(), BoundaryError> {
    let source_vertices = insert_chunks(
        artifacts,
        "boundary/sampled-vertices",
        &sheet.job.vertices,
        ChunkKind::Array,
    )?;
    let vertices = insert_chunks(
        artifacts,
        "boundary/vertices",
        &sheet.vertices,
        ChunkKind::Array,
    )?;
    let units = insert_chunks(
        artifacts,
        "boundary/units",
        &units(input, boundaries)?,
        ChunkKind::Array,
    )?;
    let geometry_kind = match trace.boundary_method {
        BoundaryMethod::Surveyed => "surveyed_polygon",
        BoundaryMethod::CompiledFromPlaces | BoundaryMethod::Unstated => "mapped_polygon",
    };
    let features = if sheet.job.target_frame.crs == "OGC:CRS84" {
        let features = geographic_features(input, trace, sheet, boundaries, geometry_kind)?;
        insert_chunks(
            artifacts,
            "boundary/features",
            &features,
            ChunkKind::GeoJson,
        )?
    } else {
        Vec::new()
    };
    insert(
        artifacts,
        "boundary/registration.json",
        &json!({
            "scan_sha256": sheet.scan_sha256, "method": sheet.job.method,
            "target_frame": sheet.job.target_frame, "source_frame": sheet.job.source_frame,
            "controls": sheet.job.controls, "check_points": sheet.job.check_points,
            "control_quality": sheet.control_quality, "check_quality": sheet.check_quality,
            "maximum_segment_pixels": spacing, "source_vertices": source_vertices,
        }),
    )?;
    insert(
        artifacts,
        "boundary.json",
        &json!({
            "version": 1, "geometry_kind": geometry_kind, "reconstructed": false,
            "source_trace": "boundary-trace.json", "registration": "boundary/registration.json",
            "target_frame": sheet.job.target_frame, "vertices": vertices,
            "units": units, "features": features, "vertex_count": sheet.vertices.len(),
            "unit_count": boundaries.len(),
        }),
    )?;
    Ok(())
}

fn geographic_features(
    input: &LedgerInput,
    trace: &SheetTrace,
    sheet: &RegisteredSheet,
    units: &[TracedUnit],
    kind: &str,
) -> Result<Vec<Value>, BoundaryError> {
    if sheet.job.target_frame.units != "degrees"
        || sheet
            .vertices
            .iter()
            .any(|p| !(-180.0..=180.0).contains(&p.x) || !(-90.0..=90.0).contains(&p.y))
    {
        return Err(BoundaryError::Evidence(
            "OGC:CRS84 requires longitude/latitude in degrees within their domains".into(),
        ));
    }
    let mut features = Vec::new();
    for unit in units {
        let record = evidence::record(input, unit.record_id)?;
        if !input
            .data
            .entities
            .iter()
            .any(|entity| entity.id == record.entity_id && entity.body == BodyId::Earth)
        {
            return Err(BoundaryError::Evidence(
                "OGC:CRS84 requires an Earth boundary".into(),
            ));
        }
        let geometry = resolve(&sheet.vertices, &unit.polygons)?;
        let mut polygons = Vec::new();
        for polygon in geometry.0 {
            let mut rings = Vec::new();
            for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
                if ring
                    .lines()
                    .any(|line| (line.start.x - line.end.x).abs() > 180.0)
                {
                    return Err(BoundaryError::Evidence(
                        "split geographic polygon edges at the antimeridian before GeoJSON export"
                            .into(),
                    ));
                }
                rings.push(
                    ring.0
                        .iter()
                        .map(|point| [point.x, point.y])
                        .collect::<Vec<_>>(),
                );
            }
            polygons.push(rings);
        }
        features.push(json!({"type": "Feature", "id": record.entity_id,
            "geometry": {"type": "MultiPolygon", "coordinates": polygons},
            "properties": {"entity_id": record.entity_id, "record_id": record.id,
                "source_id": trace.source_id, "page_or_sheet": record.page_or_sheet,
                "scan_page": trace.scan_page, "geometry_kind": kind, "reconstructed": false,
                "boundary_method": trace.boundary_method, "method_basis": trace.method_basis,
                "rank": record.observation.rank, "validity": null,
                "registration": "boundary/registration.json"}}));
    }
    Ok(features)
}
