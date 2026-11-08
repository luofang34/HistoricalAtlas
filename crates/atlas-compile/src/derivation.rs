//! Evidence-resolved terrain partitions with reproducible inputs and exclusions.

use crate::{
    export::{Artifacts, ExportError, compile_with_derived, insert, write_blocking},
    input::{InputError, LedgerInput, load_blocking, read_json_blocking},
};
use atlas_boundary::terrain::{
    DerivedBoundary, ElevationGrid, TerrainError, TerrainRequest, derive,
};
use atlas_core::identity::FactId;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};
use thiserror::Error;

mod areas;
mod evidence;

/// Checked source area and the permitted residual of its numerical fit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaConstraint {
    /// Checked territory observation for this administrative unit and edition.
    pub record_id: FactId,
    /// Maximum absolute area residual in square kilometres.
    pub tolerance_square_km: f64,
}

/// A checked observation or registered map location supplying one member position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnchorReference {
    /// Source table coordinate.
    Record {
        /// Checked coordinate record.
        record_id: FactId,
    },
    /// Location registered from a source map.
    MapLocation {
        /// Published map-location estimate.
        estimate_id: FactId,
    },
}

/// Administrative unit and the member positions used to constrain its territory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationUnit {
    /// Checked unit record from the selected gazetteer edition.
    pub record_id: FactId,
    /// Selected commune or subordinate-place coordinates.
    pub anchors: Vec<AnchorReference>,
    /// Optional source area used to constrain the terrain partition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area_constraint: Option<AreaConstraint>,
}

/// Coordinate evidence excluded from this particular reconstruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludedAnchor {
    /// Original checked observation, retained in the published ledger.
    pub record_id: FactId,
    /// Concrete reason this location does not constrain the partition.
    pub reason: String,
}

/// One source elevation tile identifying the numerical terrain input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainTile {
    /// Public source-tile URL.
    pub url: String,
    /// SHA-256 of the downloaded GeoTIFF.
    pub sha256: String,
}

/// Full recipe for a clipped terrain-cost partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationRequest {
    /// Lowercase ASCII publication name.
    pub id: String,
    /// Gazetteer edition supplying the selected administrative hierarchy.
    pub source_id: String,
    /// Registered source-map boundary enclosing the partition.
    pub envelope_record_id: FactId,
    /// Catalogued elevation dataset.
    pub terrain_source_id: String,
    /// Original numerical tile fingerprints and URLs.
    pub terrain_tiles: Vec<TerrainTile>,
    /// Cropped elevation samples and their GeoTIFF georeferencing.
    pub grid: ElevationGrid,
    /// How historical coordinates are aligned with the modern terrain frame.
    pub alignment: String,
    /// Extra horizontal-equivalent metres per metre of elevation change.
    pub slope_weight: f64,
    /// Ordered units and their selected seed evidence.
    pub units: Vec<DerivationUnit>,
    /// Reviewed locations explicitly excluded from the seed set.
    pub excluded: Vec<ExcludedAnchor>,
}

/// Derive polygons only after resolving every seed and its administrative membership.
pub fn reconstruct(
    input: &LedgerInput,
    request: &DerivationRequest,
) -> Result<Vec<DerivedBoundary>, DerivationError> {
    evidence::validate(input, request)?;
    let envelope = crate::place_maps::register_boundaries(input)?
        .into_iter()
        .find(|b| b.unit.record_id == request.envelope_record_id)
        .ok_or_else(|| {
            DerivationError::Evidence("envelope has no registered source boundary".into())
        })?;
    let (seeds, mut bases) = evidence::seeds(input, request, envelope.datum)?;
    let areas = areas::resolve(input, request, &mut bases)?;
    let neighbors = areas::neighbors(input, request, &mut bases)?;
    let numeric = TerrainRequest {
        grid: request.grid.clone(),
        vertices: envelope.vertices,
        envelope: envelope.unit.polygons,
        seeds,
        units: request.units.len(),
        slope_weight: request.slope_weight,
        areas,
        neighbors,
    };
    let mut result = Vec::new();
    for region in derive(&numeric)? {
        let unit = request
            .units
            .get(region.unit)
            .ok_or(TerrainError::Cell(region.unit))?;
        let basis = bases
            .get_mut(region.unit)
            .ok_or(TerrainError::Cell(region.unit))?;
        basis.insert(request.envelope_record_id);
        let boundary = DerivedBoundary {
            record_id: unit.record_id,
            basis: basis.iter().copied().collect(),
            region,
            datum: envelope.datum,
            cell_m: request.grid.cell_m,
            slope_weight: request.slope_weight,
            model: format!("derived/{}.json", request.id),
            comparisons: Vec::new(),
        };
        boundary.feature(&input.data, None)?;
        result.push(boundary);
    }
    atlas_boundary::terrain::compare(&input.data, &mut result)?;
    areas::annotate(request, &mut result)?;
    Ok(result)
}

/// Publish the recipe, reconstructed polygons, and the original evidence indexes.
pub fn compile_derivation(
    input: &LedgerInput,
    request: &DerivationRequest,
) -> Result<Artifacts, DerivationError> {
    let boundaries = reconstruct(input, request)?;
    let mut artifacts = compile_with_derived(input, &boundaries)?;
    insert(
        &mut artifacts,
        &format!("derived/{}.json", request.id),
        request,
    )?;
    Ok(artifacts)
}

/// Load a recipe and write a new publication directory.
pub fn derive_blocking(
    root: &Path,
    request: &Path,
    output: &Path,
) -> Result<usize, DerivationError> {
    let input = load_blocking(root)?;
    let recipe = read_json_blocking(request)?;
    let artifacts = compile_derivation(&input, &recipe)?;
    write_blocking(output, &artifacts)?;
    Ok(artifacts.len())
}

/// Recipe, source evidence, terrain geometry, or publication failure.
#[derive(Debug, Error)]
pub enum DerivationError {
    /// Input document cannot be loaded.
    #[error("derivation input: {0}")]
    Input(#[from] InputError),
    /// Seed membership, datum, or source metadata is inconsistent.
    #[error("derivation evidence: {0}")]
    Evidence(String),
    /// Source boundary registration failed.
    #[error("derivation envelope: {0}")]
    Map(#[from] crate::place_maps::PlaceMapError),
    /// Numeric grid partition or vector topology failed.
    #[error(transparent)]
    Terrain(#[from] TerrainError),
    /// Static artifact validation or writing failed.
    #[error("derivation export: {0}")]
    Export(#[from] ExportError),
}

#[cfg(test)]
mod tests;
