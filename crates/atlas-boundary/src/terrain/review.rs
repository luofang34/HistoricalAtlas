use super::{DerivedBoundary, TerrainError};
use atlas_core::identity::{EntityId, FactId};
use atlas_ledger::model::{Dataset, Record};
use geo::{
    GeodesicArea, MultiPolygon, Relate, coordinate_position::CoordPos, dimensions::Dimensions,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A model's area and adjoining units compared with one checked source passage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryComparison {
    /// Checked source description supplying the area and named neighbors.
    pub record_id: FactId,
    /// WGS84 ellipsoid footprint in the current angular frame, rounded to square metres.
    pub modeled_area_square_km: f64,
    /// Area printed in the source passage.
    pub reported_area_square_km: f64,
    /// Absolute residual permitted when this source area was used as a fitting target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fitting_tolerance_square_km: Option<f64>,
    /// Shared-edge checks for the explicitly named adjoining units.
    pub neighbors: Vec<NeighborComparison>,
}

/// Whether a named adjoining unit shares an edge in the reconstructed model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeighborComparison {
    /// Name retained from the source passage.
    pub name: String,
    /// Linked unit identity, when established.
    pub entity_id: Option<EntityId>,
    /// Unknown when the adjoining unit has no polygon in this reconstruction.
    pub shares_edge: Option<bool>,
    /// Conflicting evidence that leaves this source adjacency unresolved by the fit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<NeighborExclusion>,
}

/// Cited evidence for retaining a neighbor claim without imposing it on the geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeighborExclusion {
    /// Named neighbor whose adjacency remains unresolved.
    pub entity_id: EntityId,
    /// Checked records including the adjacency claim and the conflicting evidence.
    pub basis: Vec<FactId>,
    /// Explanation of the conflict in the cited evidence.
    pub reason: String,
}

impl NeighborExclusion {
    /// Require a named neighbor and distinct, checked evidence from the same source.
    pub fn validate(&self, data: &Dataset, record_id: FactId) -> Result<(), TerrainError> {
        let record = data.records.iter().find(|r| r.id == record_id && r.checked);
        let Some(record) = record else {
            return Err(TerrainError::Input(
                "unresolved adjacency lacks a checked claim".into(),
            ));
        };
        let basis: BTreeSet<_> = self.basis.iter().copied().collect();
        if self.reason.trim().is_empty()
            || self.entity_id == record.entity_id
            || basis.len() < 2
            || basis.len() != self.basis.len()
            || !basis.contains(&record_id)
            || !record.observation.territory.as_ref().is_some_and(|t| {
                t.neighbors
                    .iter()
                    .any(|n| n.entity_id == Some(self.entity_id))
            })
            || basis.iter().any(|id| {
                !data
                    .records
                    .iter()
                    .any(|r| r.id == *id && r.checked && r.source_id == record.source_id)
            })
        {
            return Err(TerrainError::Input(
                "unresolved adjacency needs a named neighbor, a reason and distinct checked source records".into(),
            ));
        }
        Ok(())
    }
}

/// Compare reconstructed geometry with checked descriptions from each unit's source.
pub fn compare(data: &Dataset, boundaries: &mut [DerivedBoundary]) -> Result<(), TerrainError> {
    let records: BTreeMap<_, _> = data.records.iter().map(|r| (r.id, r)).collect();
    let mut geometry = BTreeMap::new();
    for boundary in boundaries.iter() {
        let record = records
            .get(&boundary.record_id)
            .ok_or_else(|| TerrainError::Input("comparison subject record is missing".into()))?;
        geometry.insert(
            record.entity_id,
            crate::polygon::resolve(&boundary.region.vertices, &boundary.region.polygons)?,
        );
    }
    for boundary in boundaries {
        let subject = records
            .get(&boundary.record_id)
            .ok_or_else(|| TerrainError::Input("comparison subject record is missing".into()))?;
        let polygon = geometry
            .get(&subject.entity_id)
            .ok_or_else(|| TerrainError::Input("comparison subject polygon is missing".into()))?;
        boundary.comparisons = records
            .values()
            .filter_map(|record| {
                (record.checked
                    && record.entity_id == subject.entity_id
                    && record.source_id == subject.source_id)
                    .then(|| comparison(record, polygon, &geometry))
                    .flatten()
            })
            .collect();
        boundary
            .basis
            .extend(boundary.comparisons.iter().map(|c| c.record_id));
        boundary.basis.sort();
        boundary.basis.dedup();
    }
    Ok(())
}

fn comparison(
    record: &Record,
    polygon: &MultiPolygon<f64>,
    geometry: &BTreeMap<EntityId, MultiPolygon<f64>>,
) -> Option<BoundaryComparison> {
    let territory = record.observation.territory.as_ref()?;
    let neighbors = territory
        .neighbors
        .iter()
        .map(|neighbor| NeighborComparison {
            name: neighbor.name.clone(),
            entity_id: neighbor.entity_id,
            unresolved: None,
            shares_edge: neighbor
                .entity_id
                .and_then(|id| geometry.get(&id))
                .map(|other| {
                    polygon
                        .relate(other)
                        .get(CoordPos::OnBoundary, CoordPos::OnBoundary)
                        == Dimensions::OneDimensional
                }),
        })
        .collect();
    Some(BoundaryComparison {
        record_id: record.id,
        modeled_area_square_km: area_square_km(polygon),
        reported_area_square_km: territory.area_square_km,
        fitting_tolerance_square_km: None,
        neighbors,
    })
}

pub(super) fn area_square_km(polygon: &MultiPolygon<f64>) -> f64 {
    // Sub-square-metre floating-point differences must not change publication bytes.
    polygon.geodesic_area_unsigned().round() / 1_000_000.
}

pub(super) fn validate(
    boundary: &DerivedBoundary,
    data: &Dataset,
    subject: EntityId,
    polygon: &MultiPolygon<f64>,
) -> Result<(), TerrainError> {
    for comparison in &boundary.comparisons {
        let territory = data
            .records
            .iter()
            .find(|r| {
                r.id == comparison.record_id
                    && r.checked
                    && r.entity_id == subject
                    && boundary.basis.contains(&r.id)
            })
            .and_then(|r| r.observation.territory.as_ref());
        let Some(territory) = territory else {
            return Err(TerrainError::Input(
                "comparison needs checked territory evidence".into(),
            ));
        };
        for neighbor in &comparison.neighbors {
            if let Some(unresolved) = &neighbor.unresolved {
                unresolved.validate(data, comparison.record_id)?;
                if neighbor.entity_id != Some(unresolved.entity_id)
                    || unresolved
                        .basis
                        .iter()
                        .any(|id| !boundary.basis.contains(id))
                {
                    return Err(TerrainError::Input(
                        "unresolved adjacency differs from its evidence".into(),
                    ));
                }
            }
        }
        if comparison.reported_area_square_km != territory.area_square_km
            || !comparison.modeled_area_square_km.is_finite()
            || comparison.modeled_area_square_km != area_square_km(polygon)
            || comparison
                .fitting_tolerance_square_km
                .is_some_and(|tolerance| {
                    !tolerance.is_finite()
                        || tolerance <= 0.
                        || tolerance >= territory.area_square_km
                        || (comparison.modeled_area_square_km - territory.area_square_km).abs()
                            > tolerance
                })
            || comparison.neighbors.len() != territory.neighbors.len()
            || comparison
                .neighbors
                .iter()
                .zip(&territory.neighbors)
                .any(|(a, b)| a.name != b.name || a.entity_id != b.entity_id)
        {
            return Err(TerrainError::Input(
                "comparison differs from its geometry or source".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
