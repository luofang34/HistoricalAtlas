//! Registered Earth boundaries in OGC:CRS84 longitude and latitude degrees.

use crate::{
    polygon::resolve,
    sheet::{BoundaryMethod, TracedUnit},
};
use atlas_core::identity::{EntityId, FactId};
use atlas_geo::{body::BodyId, registration::PlanarPoint};
use atlas_ledger::model::{Dataset, Record};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

/// One registered source-map boundary and its evidence links.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeographicBoundary {
    /// Checked sheet observation and indexed polygon components.
    pub unit: TracedUnit,
    /// Longitude and latitude degrees in OGC:CRS84.
    pub vertices: Vec<PlanarPoint>,
    /// How the source constructed its boundary.
    pub boundary_method: BoundaryMethod,
    /// Same-edition observations establishing the source's method.
    pub method_basis: Vec<FactId>,
    /// Publication-relative path to control references and registration residuals.
    pub registration: String,
}

impl GeographicBoundary {
    /// Validate evidence and topology, then emit a cited GeoJSON feature.
    pub fn feature(
        &self,
        data: &Dataset,
        selected: Option<EntityId>,
    ) -> Result<Value, GeographicError> {
        let record = self.record(data)?;
        if self
            .vertices
            .iter()
            .any(|p| !(-180.0..=180.0).contains(&p.x) || !(-90.0..=90.0).contains(&p.y))
        {
            return Err(GeographicError::Evidence(
                "OGC:CRS84 coordinates exceed their domains".into(),
            ));
        }
        let geometry = resolve(&self.vertices, &self.unit.polygons)?;
        let mut polygons = Vec::new();
        for polygon in geometry.0 {
            let mut rings = Vec::new();
            for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
                if ring
                    .lines()
                    .any(|line| (line.start.x - line.end.x).abs() > 180.0)
                {
                    return Err(GeographicError::Evidence(
                        "split geographic polygon edges at the antimeridian".into(),
                    ));
                }
                rings.push(ring.0.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>());
            }
            polygons.push(rings);
        }
        let kind = match self.boundary_method {
            BoundaryMethod::Surveyed => "surveyed_polygon",
            BoundaryMethod::CompiledFromPlaces | BoundaryMethod::Unstated => "mapped_polygon",
        };
        Ok(json!({"type": "Feature", "id": record.entity_id,
            "geometry": {"type": "MultiPolygon", "coordinates": polygons},
            "properties": {"entity_id": record.entity_id, "record_id": record.id,
                "name": record.observation.name, "source_id": record.source_id,
                "page_or_sheet": record.page_or_sheet, "scan_page": record.scan_page,
                "geometry_role": "registered_boundary", "geometry_kind": kind,
                "selected": selected == Some(record.entity_id), "reconstructed": false,
                "boundary_method": self.boundary_method, "method_basis": self.method_basis,
                "rank": record.observation.rank, "validity": null,
                "registration": self.registration}}))
    }

    fn record<'a>(&self, data: &'a Dataset) -> Result<&'a Record, GeographicError> {
        let record = data
            .records
            .iter()
            .find(|r| r.id == self.unit.record_id && r.checked)
            .ok_or_else(|| {
                GeographicError::Evidence(format!(
                    "missing checked boundary record {:?}",
                    self.unit.record_id
                ))
            })?;
        if record.scan_page.is_none()
            || !data
                .entities
                .iter()
                .any(|e| e.id == record.entity_id && e.body == BodyId::Earth)
        {
            return Err(GeographicError::Evidence(
                "OGC:CRS84 requires a cited Earth map boundary".into(),
            ));
        }
        if self.boundary_method != BoundaryMethod::Unstated && self.method_basis.is_empty() {
            return Err(GeographicError::Evidence(
                "stated boundary method requires evidence".into(),
            ));
        }
        for id in &self.method_basis {
            if !data
                .records
                .iter()
                .any(|r| r.id == *id && r.checked && r.source_id == record.source_id)
            {
                return Err(GeographicError::Evidence(format!(
                    "method record {id:?} does not cite the boundary edition"
                )));
            }
        }
        if self.registration.is_empty()
            || self
                .registration
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            || self.registration.contains(['\\', ':', '?', '#'])
            || !self.registration.ends_with(".json")
        {
            return Err(GeographicError::Evidence(
                "registration must be a publication-relative JSON path".into(),
            ));
        }
        Ok(record)
    }
}

/// Invalid registered polygon or its source evidence.
#[derive(Debug, Error)]
pub enum GeographicError {
    /// Geometry is not a valid polygon or multipolygon.
    #[error("registered boundary topology: {0}")]
    Topology(#[from] crate::polygon::TopologyError),
    /// Boundary evidence or geographic frame is inconsistent.
    #[error("registered boundary evidence: {0}")]
    Evidence(String),
}
