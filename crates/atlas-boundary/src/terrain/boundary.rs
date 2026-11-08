use super::{TerrainError, TerrainRegion};
use atlas_core::identity::{EntityId, FactId};
use atlas_geo::{body::BodyId, coordinates::Datum};
use atlas_ledger::model::Dataset;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Reconstructed territorial extent, separate from source-map observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedBoundary {
    /// Checked record identifying the administrative unit.
    pub record_id: FactId,
    /// Input records supporting membership, locations, and the enclosing boundary.
    pub basis: Vec<FactId>,
    /// Terrain-cost partition geometry.
    pub region: TerrainRegion,
    /// Datum retained from the historical coordinate evidence.
    pub datum: Datum,
    /// Projected grid cell size in metres.
    pub cell_m: f64,
    /// Elevation-change cost coefficient used by the model.
    pub slope_weight: f64,
    /// Publication-relative model document containing all inputs and exclusions.
    pub model: String,
}

impl DerivedBoundary {
    /// Validate subject, evidence, and polygon topology before drawing a reconstruction.
    pub fn feature(
        &self,
        data: &Dataset,
        selected: Option<EntityId>,
    ) -> Result<Value, TerrainError> {
        let record = data
            .records
            .iter()
            .find(|r| r.id == self.record_id && r.checked)
            .ok_or_else(|| {
                TerrainError::Input("derived boundary needs a checked unit record".into())
            })?;
        if !data
            .entities
            .iter()
            .any(|e| e.id == record.entity_id && e.body == BodyId::Earth)
            || !self.basis.contains(&self.record_id)
            || self
                .basis
                .iter()
                .any(|id| !data.records.iter().any(|r| r.id == *id && r.checked))
            || !self.cell_m.is_finite()
            || self.cell_m <= 0.0
            || !self.slope_weight.is_finite()
            || self.slope_weight < 0.0
        {
            return Err(TerrainError::Input(
                "invalid reconstruction subject, evidence, or parameters".into(),
            ));
        }
        self.datum
            .validate_body(BodyId::Earth)
            .map_err(|e| TerrainError::Input(e.to_string()))?;
        if !self.model.ends_with(".json")
            || self.model.contains(['\\', ':', '?', '#'])
            || self
                .model
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || self
                .region
                .vertices
                .iter()
                .any(|p| !(-180.0..=180.0).contains(&p.x) || !(-85.0..=85.0).contains(&p.y))
        {
            return Err(TerrainError::Input(
                "invalid reconstruction model path or angular domain".into(),
            ));
        }
        let geometry = crate::polygon::resolve(&self.region.vertices, &self.region.polygons)?;
        let polygons: Vec<_> = geometry
            .0
            .iter()
            .map(|p| {
                std::iter::once(p.exterior())
                    .chain(p.interiors())
                    .map(|r| r.0.iter().map(|c| [c.x, c.y]).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            })
            .collect();
        Ok(json!({"type":"Feature","id":record.entity_id,
            "geometry":{"type":"MultiPolygon","coordinates":polygons},
            "properties":{"entity_id":record.entity_id,"record_id":self.record_id,
                "name":record.observation.name,"rank":record.observation.rank,
                "geometry_role":"derived_boundary","geometry_kind":"derived_polygon",
                "reconstructed":true,"selected":selected==Some(record.entity_id),
                "datum":self.datum,"cell_m":self.cell_m,"slope_weight":self.slope_weight,
                "model":self.model,"basis":self.basis,"validity":null}}))
    }
}
