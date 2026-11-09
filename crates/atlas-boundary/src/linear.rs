//! Cited open linework and its registered geographic representation.

use atlas_core::{
    identity::{EntityId, FactId},
    time::TemporalSpan,
};
use atlas_geo::{coordinates::Datum, registration::PlanarPoint};
use atlas_ledger::model::Dataset;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

mod sampling;
pub use sampling::densify;

/// Open or closed paths sharing a source sheet's vertex array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TracedLine {
    /// Checked observation identifying the depicted feature.
    pub record_id: FactId,
    /// Connected components, each containing at least two vertex indices.
    pub paths: Vec<Vec<usize>>,
    /// Evidence-supported interval for this particular geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validity: Option<TemporalSpan>,
}

/// Geographic linework retaining its source observation and registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeographicLine {
    /// Subject, indexed paths, and validity interval.
    pub line: TracedLine,
    /// Registered longitude and latitude in degrees.
    pub vertices: Vec<PlanarPoint>,
    /// Coordinate reference datum on the subject's celestial body.
    pub datum: Datum,
    /// Publication-relative registration evidence.
    pub registration: String,
}

impl GeographicLine {
    /// Validate and emit a cited GeoJSON MultiLineString.
    pub fn feature(&self, data: &Dataset, selected: Option<EntityId>) -> Result<Value, LineError> {
        let record = data
            .records
            .iter()
            .find(|r| r.id == self.line.record_id && r.checked)
            .ok_or_else(|| {
                LineError::Evidence(format!(
                    "missing checked line record {:?}",
                    self.line.record_id
                ))
            })?;
        let entity = data
            .entities
            .iter()
            .find(|e| e.id == record.entity_id)
            .ok_or_else(|| {
                LineError::Evidence(format!("missing subject {:?}", record.entity_id))
            })?;
        self.datum.validate_body(entity.body)?;
        validate(&self.vertices, &self.line)?;
        if self
            .vertices
            .iter()
            .any(|p| !(-180.0..=180.0).contains(&p.x) || !(-90.0..=90.0).contains(&p.y))
        {
            return Err(LineError::Evidence(
                "line coordinates exceed the geographic domain".into(),
            ));
        }
        if self.registration.is_empty()
            || !self.registration.ends_with(".json")
            || self.registration.contains(['\\', ':', '?', '#'])
            || self
                .registration
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
        {
            return Err(LineError::Evidence(
                "registration must be a publication-relative JSON path".into(),
            ));
        }
        let paths = resolve(&self.vertices, &self.line.paths)?;
        if paths.iter().any(|path| {
            path.iter()
                .zip(path.iter().skip(1))
                .any(|(a, b)| (a[0] - b[0]).abs() > 180.0)
        }) {
            return Err(LineError::Evidence(
                "split geographic line edges at the antimeridian".into(),
            ));
        }
        Ok(json!({"type":"Feature","id":record.entity_id,
            "geometry":{"type":"MultiLineString","coordinates":paths},
            "properties":{"entity_id":record.entity_id,"record_id":record.id,"name":record.observation.name,
                "entity_kind":entity.kind,"source_id":record.source_id,"page_or_sheet":record.page_or_sheet,
                "scan_page":record.scan_page,"geometry_role":"registered_line","geometry_kind":"mapped_line",
                "selected":selected==Some(record.entity_id),"reconstructed":false,"datum":self.datum,
                "validity":self.line.validity,"registration":self.registration}}))
    }
}

/// Validate indices, finite coordinates, nonzero edges, and the declared interval.
pub fn validate(vertices: &[PlanarPoint], line: &TracedLine) -> Result<(), LineError> {
    resolve(vertices, &line.paths)?;
    if let Some(span) = &line.validity {
        span.validate()?;
    }
    Ok(())
}

/// Resolve paths without closing open linework or joining separate components.
pub fn resolve(
    vertices: &[PlanarPoint],
    paths: &[Vec<usize>],
) -> Result<Vec<Vec<[f64; 2]>>, LineError> {
    if paths.is_empty() {
        return Err(LineError::Path);
    }
    paths
        .iter()
        .map(|path| {
            if path.len() < 2 {
                return Err(LineError::Path);
            }
            let points = path
                .iter()
                .map(|index| {
                    let point = vertices.get(*index).ok_or(LineError::Index(*index))?;
                    if !point.x.is_finite() || !point.y.is_finite() {
                        return Err(LineError::Coordinate(*index));
                    }
                    Ok([point.x, point.y])
                })
                .collect::<Result<Vec<_>, _>>()?;
            if points
                .iter()
                .zip(points.iter().skip(1))
                .any(|(a, b)| a == b)
            {
                return Err(LineError::Path);
            }
            Ok(points)
        })
        .collect()
}

/// Invalid open geometry, registration, or evidence.
#[derive(Debug, Error)]
pub enum LineError {
    /// A connected path has fewer than two positions or a zero-length edge.
    #[error("linework requires nonempty paths with at least two distinct consecutive positions")]
    Path,
    /// An index references an absent vertex.
    #[error("missing line vertex {0}")]
    Index(usize),
    /// A vertex contains a non-finite value.
    #[error("line vertex {0} is non-finite")]
    Coordinate(usize),
    /// Sampling would exceed the supported budget.
    #[error("line sampling requires finite positive spacing and at most 1000000 vertices")]
    Sampling,
    /// Source or registration evidence is inconsistent.
    #[error("line evidence: {0}")]
    Evidence(String),
    /// Coordinate reference frame does not match the subject.
    #[error(transparent)]
    Datum(#[from] atlas_geo::coordinates::CoordinateError),
    /// Geometry validity is inconsistent.
    #[error(transparent)]
    Time(#[from] atlas_core::time::TimeError),
}

#[cfg(test)]
mod tests;
