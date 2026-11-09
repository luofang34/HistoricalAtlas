//! Pinhole-camera registration of photographs over cited terrain.

use atlas_boundary::terrain::ElevationGrid;
use atlas_geo::{
    body::BodyId,
    camera::{
        CameraControl, CameraError, CameraPose, PinholeCamera, ResectionOptions, Terrain,
        ViewAngles, intersect, resect,
    },
    coordinates::{Datum, LonLat},
    geodesy::{GeodeticPoint, LocalFrame},
    registration::{ControlPoint, ControlSupport, PlanarPoint, RegistrationError},
};
use serde::{Deserialize, Serialize};

use super::{ReferencedControl, RegistrationJob, RegistrationJobError};
use crate::derivation::TerrainTile;
use crate::input::LedgerInput;
use atlas_ledger::model::SourceCatalog;

/// Elevation model under a photographed scene, with its source tiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationTerrain {
    /// Catalogued elevation source.
    pub source_id: String,
    /// Downloaded tiles from which the grid was cropped.
    pub tiles: Vec<TerrainTile>,
    /// Cropped elevation samples covering every control and vertex.
    pub grid: ElevationGrid,
    /// Height of open water in metres; lower samples are seafloor, which a
    /// photograph sees through the water surface rather than at depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_surface_m: Option<f64>,
}

/// A reusable `*.terrain.json` document, named by its data-relative path without the suffix.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedTerrain {
    /// Such as `guadalcanal-1942/lunga` for `data/guadalcanal-1942/lunga.terrain.json`.
    pub terrain_id: String,
}

/// Terrain for a registration: a shared document, so photographs of one area
/// cite one grid, or a grid embedded in a standalone job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TerrainInput {
    /// Reference to a loaded terrain document.
    Shared(SharedTerrain),
    /// Embedded grid and provenance.
    Inline(Box<RegistrationTerrain>),
}

/// Check a terrain document's source rights, tile fingerprints and samples.
pub fn validate_terrain(
    catalog: &SourceCatalog,
    terrain: &RegistrationTerrain,
) -> Result<(), RegistrationJobError> {
    let evidence = |message: String| RegistrationJobError::Evidence(message);
    if !catalog
        .sources
        .iter()
        .any(|source| source.id == terrain.source_id && source.rights.is_some())
    {
        return Err(evidence(format!(
            "terrain source {} must be catalogued with a rights statement",
            terrain.source_id
        )));
    }
    let digest = |value: &str| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit());
    if terrain.tiles.is_empty() || !terrain.tiles.iter().all(|tile| digest(&tile.sha256)) {
        return Err(evidence(
            "terrain grid requires fingerprinted source tiles".into(),
        ));
    }
    if terrain
        .water_surface_m
        .is_some_and(|height| !height.is_finite())
    {
        return Err(evidence("water surface height must be finite".into()));
    }
    terrain
        .grid
        .validate()
        .map_err(|error| evidence(error.to_string()))
}

/// The surface a camera sees: terrain, raised to the water surface where submerged.
///
/// Elevation grids use Web Mercator cells scaled by the Earth's radius on every
/// body, so their ground spacing is the cell size times the body's radius over
/// the Earth's.
#[derive(Debug, Clone)]
struct VisibleSurface {
    grid: ElevationGrid,
    water_surface_m: Option<f64>,
    spacing_scale: f64,
}

impl Terrain for VisibleSurface {
    fn height_m(&self, position: LonLat) -> Option<f64> {
        let ground = self.grid.height_m(position)?;
        Some(
            self.water_surface_m
                .map_or(ground, |water| ground.max(water)),
        )
    }

    fn spacing_m(&self) -> f64 {
        self.grid.spacing_m() * self.spacing_scale
    }
}

/// Fitted camera geometry published with a registration for review.
#[derive(Debug, Clone, Serialize)]
pub struct CameraReport {
    /// Estimated or fixed focal length in pixels.
    pub focal_px: f64,
    /// Relative focal-length spread per pixel of control error, when estimated.
    pub focal_spread_per_px: Option<f64>,
    /// Principal point in image pixels.
    pub principal: PlanarPoint,
    /// Optical centre: longitude and latitude in degrees, height in metres.
    pub centre: [f64; 3],
    /// Heading, depression and roll of the optical axis.
    pub view: ViewAngles,
    /// Reprojection error at fitting controls, in pixels.
    pub control_residuals_px: Vec<f64>,
    /// Reprojection error at check points, in pixels.
    pub check_residuals_px: Vec<f64>,
}

#[derive(Debug)]
pub(super) struct FittedCamera {
    frame: LocalFrame,
    terrain: VisibleSurface,
    camera: PinholeCamera,
    pose: CameraPose,
    support: ControlSupport,
    pub(super) report: CameraReport,
}

impl FittedCamera {
    pub(super) fn fit(
        input: &LedgerInput,
        job: &RegistrationJob,
        focal_px: Option<f64>,
    ) -> Result<Self, RegistrationJobError> {
        let (validated, body) = validated_terrain(input, job)?;
        let ellipsoid = body.ellipsoid();
        let terrain = VisibleSurface {
            grid: validated.grid.clone(),
            water_surface_m: validated.water_surface_m,
            spacing_scale: ellipsoid.semi_major_m / BodyId::Earth.ellipsoid().semi_major_m,
        };
        let origin = centroid(&job.controls)?;
        let frame = LocalFrame::with_ellipsoid(ellipsoid, origin);
        let ground = |control: &ReferencedControl| -> Result<CameraControl, RegistrationJobError> {
            let position = LonLat {
                longitude: control.point.target.x,
                latitude: control.point.target.y,
            };
            let height_m = terrain.height_m(position).ok_or_else(|| {
                RegistrationJobError::Evidence(format!(
                    "control {} lies outside the terrain grid",
                    control.landmark
                ))
            })?;
            Ok(CameraControl {
                pixel: control.point.source,
                world: frame.to_local(GeodeticPoint { position, height_m }),
            })
        };
        let controls: Vec<_> = job.controls.iter().map(ground).collect::<Result<_, _>>()?;
        let checks: Vec<_> = job
            .check_points
            .iter()
            .map(ground)
            .collect::<Result<_, _>>()?;
        let principal = PlanarPoint {
            x: (f64::from(job.source_frame.width) - 1.0) / 2.0,
            y: (f64::from(job.source_frame.height) - 1.0) / 2.0,
        };
        let fit = resect(
            &controls,
            &ResectionOptions {
                principal,
                focal_px,
            },
        )
        .map_err(camera_error)?;
        let check_residuals_px = checks
            .iter()
            .map(|check| {
                fit.pose
                    .project(&fit.camera, check.world)
                    .map_or(f64::INFINITY, |pixel| pixel.distance(check.pixel))
            })
            .collect();
        let centre = frame.to_geodetic(fit.pose.centre);
        let planar: Vec<ControlPoint> = job.controls.iter().map(|c| c.point).collect();
        Ok(Self {
            report: CameraReport {
                focal_px: fit.camera.focal_px,
                focal_spread_per_px: fit.focal_spread_per_px,
                principal,
                centre: [
                    centre.position.longitude,
                    centre.position.latitude,
                    centre.height_m,
                ],
                view: fit.pose.view_angles(),
                control_residuals_px: fit.residuals_px,
                check_residuals_px,
            },
            support: ControlSupport::new(&planar)?,
            frame,
            terrain,
            camera: fit.camera,
            pose: fit.pose,
        })
    }

    /// Ground position of a pixel inside the control hull, as longitude and latitude.
    pub(super) fn transform(&self, pixel: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        self.support.require(pixel)?;
        let ground = intersect(&self.frame, &self.terrain, &self.camera, &self.pose, pixel)
            .map_err(|error| match error {
                CameraError::NoGround(point) => RegistrationError::BeyondHorizon(point),
                _ => RegistrationError::Solve("camera ray intersection"),
            })?;
        Ok(PlanarPoint {
            x: ground.position.longitude,
            y: ground.position.latitude,
        })
    }
}

fn validated_terrain<'a>(
    input: &'a LedgerInput,
    job: &'a RegistrationJob,
) -> Result<(&'a RegistrationTerrain, BodyId), RegistrationJobError> {
    let evidence = |message: String| RegistrationJobError::Evidence(message);
    let body = job
        .target_frame
        .geographic_datum()?
        .and_then(Datum::body)
        .ok_or_else(|| {
            evidence(
                "camera registration requires longitude and latitude on a declared body datum"
                    .into(),
            )
        })?;
    let terrain = match job.terrain.as_ref() {
        None => {
            return Err(evidence(
                "camera registration requires a terrain grid".into(),
            ));
        }
        Some(TerrainInput::Inline(terrain)) => terrain.as_ref(),
        Some(TerrainInput::Shared(shared)) => input
            .terrains
            .get(&shared.terrain_id)
            .ok_or_else(|| evidence(format!("unknown terrain document {}", shared.terrain_id)))?,
    };
    validate_terrain(&input.catalog, terrain)?;
    Ok((terrain, body))
}

fn centroid(controls: &[ReferencedControl]) -> Result<GeodeticPoint, RegistrationJobError> {
    let count = controls.len() as f64;
    let position = LonLat {
        longitude: controls.iter().map(|c| c.point.target.x).sum::<f64>() / count,
        latitude: controls.iter().map(|c| c.point.target.y).sum::<f64>() / count,
    };
    if !position.longitude.is_finite() || !position.latitude.is_finite() {
        return Err(RegistrationJobError::Evidence(
            "camera registration requires controls".into(),
        ));
    }
    Ok(GeodeticPoint {
        position,
        height_m: 0.0,
    })
}

fn camera_error(error: CameraError) -> RegistrationJobError {
    match error {
        CameraError::Registration(error) => RegistrationJobError::Fit(error),
        other => RegistrationJobError::Evidence(other.to_string()),
    }
}

#[cfg(test)]
mod tests;
