//! Reproducible registration jobs linked to gazetteer sheets and control references.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use atlas_geo::registration::{
    ControlPoint, PlanarPoint, Registration, RegistrationError, RegistrationQuality,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
mod fitted;

use crate::{
    export::MAX_FILE_BYTES,
    input::{InputError, LedgerInput, load_blocking, read_json_blocking},
};

/// A landmark correspondence with an independently traceable target reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencedControl {
    /// Checked ledger facts establishing this correspondence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub basis: Vec<atlas_core::identity::FactId>,
    /// Landmark name as identified on the sheet.
    pub landmark: String,
    /// Survey, dataset feature, or bibliographic locator establishing the target.
    pub reference: String,
    /// SHA-256 of the reference raster or dataset on which the target was measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_sha256: Option<String>,
    /// Stated identification uncertainty of the target, in target units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<f64>,
    /// Source image pixels and target planar coordinates.
    pub point: ControlPoint,
}

/// Registration method and its parameters.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegistrationMethod {
    /// Least-squares affine fit.
    #[default]
    Affine,
    /// Thin-plate spline warp.
    ThinPlate {
        /// Nonnegative smoothing in normalized source units.
        smoothing: f64,
    },
    /// Planar homography for photographs of level ground, evaluated only inside the control hull.
    Projective,
}

/// Coordinate frame declared for every target control and output point.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetFrame {
    /// Geographic datum for angular output on a named celestial body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datum: Option<atlas_geo::coordinates::Datum>,
    /// CRS identifier or complete definition, such as an EPSG identifier.
    pub crs: String,
    /// Coordinate units; residuals use these same units.
    pub units: String,
}

impl TargetFrame {
    /// Geographic publication datum; projected frames retain their native coordinates.
    pub fn geographic_datum(
        &self,
    ) -> Result<Option<atlas_geo::coordinates::Datum>, RegistrationJobError> {
        use atlas_geo::coordinates::Datum;
        if (self.crs == "OGC:CRS84" || self.datum.is_some()) && self.units != "degrees" {
            return Err(RegistrationJobError::Evidence(
                "geographic output requires degree units".into(),
            ));
        }
        if self.crs == "OGC:CRS84" {
            if self.datum.is_some_and(|datum| datum != Datum::Wgs84) {
                return Err(RegistrationJobError::Evidence(
                    "OGC:CRS84 requires the WGS84 datum".into(),
                ));
            }
            return Ok(Some(Datum::Wgs84));
        }
        Ok(self.datum)
    }
}

pub use atlas_boundary::sheet::RasterFrame;

/// Complete inputs for fitting a source sheet and transforming traced vertices.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationJob {
    /// Catalogued source edition.
    pub source_id: String,
    /// One-based page for a paginated scan; absent for a fingerprinted capture.
    pub scan_page: Option<u32>,
    /// Full-page image dimensions defining the source pixel frame.
    pub source_frame: RasterFrame,
    /// Declared target coordinate system.
    pub target_frame: TargetFrame,
    /// Fitting method.
    pub method: RegistrationMethod,
    /// Controls included in the fit.
    pub controls: Vec<ReferencedControl>,
    /// Independent check points excluded from the fit.
    pub check_points: Vec<ReferencedControl>,
    /// Source image vertices to transform, in the controls' pixel frame.
    pub vertices: Vec<PlanarPoint>,
}

/// A sheet registration result with provenance and measured fit quality.
#[derive(Debug, Serialize)]
pub struct RegisteredSheet {
    #[serde(skip)]
    model: fitted::Fitted,
    /// Catalogued fingerprint of the source scan used for this result.
    pub scan_sha256: String,
    /// Input controls, references, and coordinate definitions.
    pub job: RegistrationJob,
    /// Errors at the fitting controls, in target units.
    pub control_quality: RegistrationQuality,
    /// Errors at independent check points, when supplied.
    pub check_quality: Option<RegistrationQuality>,
    /// Transformed vertices, in input order and target units.
    pub vertices: Vec<PlanarPoint>,
}

impl RegisteredSheet {
    /// Transform additional geometry with the fitted model and original control solution.
    pub fn transform(&self, point: PlanarPoint) -> Result<PlanarPoint, RegistrationError> {
        self.model.transform(point)
    }
}

/// Fit an identified sheet and evaluate its independent control evidence.
pub fn register(
    input: &LedgerInput,
    job: RegistrationJob,
) -> Result<RegisteredSheet, RegistrationJobError> {
    let scan_sha256 = validate_job(input, &job)?;
    let controls: Vec<_> = job.controls.iter().map(|control| control.point).collect();
    let model = fitted::Fitted::fit(job.method, &controls)?;
    let control_quality = model.evaluate(&controls)?;
    let check_quality = if job.check_points.is_empty() {
        None
    } else {
        let checks: Vec<_> = job
            .check_points
            .iter()
            .map(|control| control.point)
            .collect();
        Some(model.evaluate(&checks)?)
    };
    let vertices = job
        .vertices
        .iter()
        .map(|point| model.transform(*point))
        .collect::<Result<_, _>>()?;
    Ok(RegisteredSheet {
        model,
        scan_sha256,
        job,
        control_quality,
        check_quality,
        vertices,
    })
}

fn validate_job(
    input: &LedgerInput,
    job: &RegistrationJob,
) -> Result<String, RegistrationJobError> {
    let source = input
        .catalog
        .sources
        .iter()
        .find(|source| source.id == job.source_id)
        .ok_or_else(|| {
            RegistrationJobError::Evidence(format!("unknown source {}", job.source_id))
        })?;
    let fingerprint = source.raster_fingerprint(job.scan_page).ok_or_else(|| {
        RegistrationJobError::Evidence(format!(
            "{} raster {:?} has no fingerprint",
            source.id, job.scan_page
        ))
    })?;
    if job.target_frame.crs.trim().is_empty() || job.target_frame.units.trim().is_empty() {
        return Err(RegistrationJobError::Evidence(
            "target CRS and units must be declared".into(),
        ));
    }
    job.target_frame.geographic_datum()?;
    validate_controls(input, job)?;
    if job.source_frame.width == 0 || job.source_frame.height == 0 {
        return Err(RegistrationJobError::Evidence(
            "source raster dimensions must be positive".into(),
        ));
    }
    for point in job
        .controls
        .iter()
        .chain(&job.check_points)
        .map(|control| control.point.source)
        .chain(job.vertices.iter().copied())
    {
        if !job.source_frame.contains(point) {
            return Err(RegistrationJobError::Evidence(format!(
                "source point {point:?} is outside the full-page raster"
            )));
        }
    }
    for check in &job.check_points {
        if job.controls.iter().any(|control| {
            control.landmark == check.landmark || control.point.source == check.point.source
        }) {
            return Err(RegistrationJobError::Evidence(format!(
                "check point {} is also a fitting control",
                check.landmark
            )));
        }
    }
    Ok(fingerprint.to_owned())
}

fn validate_controls(
    input: &LedgerInput,
    job: &RegistrationJob,
) -> Result<(), RegistrationJobError> {
    for control in job.controls.iter().chain(&job.check_points) {
        for id in &control.basis {
            if !input
                .data
                .records
                .iter()
                .any(|record| record.id == *id && record.checked)
            {
                return Err(RegistrationJobError::Evidence(format!(
                    "control {} references missing or unchecked fact {id:?}",
                    control.landmark
                )));
            }
        }
        if control.landmark.trim().is_empty() || control.reference.trim().is_empty() {
            return Err(RegistrationJobError::Evidence(
                "every control requires a landmark and target reference".into(),
            ));
        }
        validate_target_evidence(control)?;
    }
    Ok(())
}

/// A target must be traceable to checked facts or to a fingerprinted reference, never asserted.
fn validate_target_evidence(control: &ReferencedControl) -> Result<(), RegistrationJobError> {
    let fingerprint = control.reference_sha256.as_deref();
    if fingerprint.is_some_and(|digest| {
        digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(RegistrationJobError::Evidence(format!(
            "control {} reference fingerprint must be a SHA-256 digest",
            control.landmark
        )));
    }
    if control.basis.is_empty() && fingerprint.is_none() {
        return Err(RegistrationJobError::Evidence(format!(
            "control {} requires checked basis facts or a fingerprinted reference",
            control.landmark
        )));
    }
    if control
        .uncertainty
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(RegistrationJobError::Evidence(format!(
            "control {} uncertainty must be finite and nonnegative",
            control.landmark
        )));
    }
    Ok(())
}

/// Read a registration job and write a new result file after fitting succeeds.
pub fn register_blocking(
    root: &Path,
    request: &Path,
    output: &Path,
) -> Result<RegisteredSheet, RegistrationJobError> {
    let input = load_blocking(root)?;
    let job = read_json_blocking(request)?;
    let result = register(&input, job)?;
    let bytes = serde_json::to_vec_pretty(&result)?;
    if bytes.len() >= MAX_FILE_BYTES {
        return Err(RegistrationJobError::Oversize(bytes.len()));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|source| RegistrationJobError::Io {
            path: output.to_owned(),
            source,
        })?;
    file.write_all(&bytes)
        .map_err(|source| RegistrationJobError::Io {
            path: output.to_owned(),
            source,
        })?;
    Ok(result)
}

/// Registration job input, evidence, fitting, or output failure.
#[derive(Debug, Error)]
pub enum RegistrationJobError {
    /// Ledger or job input failed.
    #[error("registration input: {0}")]
    Input(#[from] InputError),
    /// Invalid source linkage or target reference.
    #[error("registration evidence: {0}")]
    Evidence(String),
    /// Numerical registration failed.
    #[error("sheet registration: {0}")]
    Fit(#[from] RegistrationError),
    /// Serialization failed.
    #[error("registration result serialization: {0}")]
    Json(#[from] serde_json::Error),
    /// Serialized result exceeds the publication limit.
    #[error("registration result is {0} bytes, exceeding the static file budget")]
    Oversize(usize),
    /// Result file could not be written.
    #[error("cannot write registration result {path}: {source}")]
    Io {
        /// Destination path.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests;
