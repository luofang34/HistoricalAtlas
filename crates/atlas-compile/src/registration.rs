//! Reproducible registration jobs linked to gazetteer sheets and control references.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use atlas_geo::registration::{
    AffineRegistration, ControlPoint, PlanarPoint, Registration, RegistrationError,
    RegistrationQuality, ThinPlateSpline,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    export::MAX_FILE_BYTES,
    input::{InputError, LedgerInput, load_blocking, read_json_blocking},
};

/// A landmark correspondence with an independently traceable target reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencedControl {
    /// Landmark name as identified on the sheet.
    pub landmark: String,
    /// Survey, dataset feature, or bibliographic locator establishing the target.
    pub reference: String,
    /// Source image pixels and target planar coordinates.
    pub point: ControlPoint,
}

/// Registration method and its parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegistrationMethod {
    /// Least-squares affine fit.
    Affine,
    /// Thin-plate spline warp.
    ThinPlate {
        /// Nonnegative smoothing in normalized source units.
        smoothing: f64,
    },
}

/// Coordinate frame declared for every target control and output point.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetFrame {
    /// CRS identifier or complete definition, such as an EPSG identifier.
    pub crs: String,
    /// Coordinate units; residuals use these same units.
    pub units: String,
}

pub use atlas_boundary::sheet::RasterFrame;

/// Complete inputs for fitting a source sheet and transforming traced vertices.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationJob {
    /// Catalogued source edition.
    pub source_id: String,
    /// One-based scan page of the sheet.
    pub scan_page: u32,
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

/// Fit an identified sheet and evaluate its independent control evidence.
pub fn register(
    input: &LedgerInput,
    job: RegistrationJob,
) -> Result<RegisteredSheet, RegistrationJobError> {
    let scan_sha256 = validate_job(input, &job)?;
    let controls: Vec<_> = job.controls.iter().map(|control| control.point).collect();
    let model: Box<dyn Registration> = match job.method {
        RegistrationMethod::Affine => Box::new(AffineRegistration::fit(&controls)?),
        RegistrationMethod::ThinPlate { smoothing } => {
            Box::new(ThinPlateSpline::fit(&controls, smoothing)?)
        }
    };
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
    if job.scan_page == 0 || source.scan_pages.is_none_or(|pages| job.scan_page > pages) {
        return Err(RegistrationJobError::Evidence(format!(
            "{} scan page {} out of range",
            source.id, job.scan_page
        )));
    }
    if job.target_frame.crs.trim().is_empty() || job.target_frame.units.trim().is_empty() {
        return Err(RegistrationJobError::Evidence(
            "target CRS and units must be declared".into(),
        ));
    }
    for control in job.controls.iter().chain(&job.check_points) {
        if control.landmark.trim().is_empty() || control.reference.trim().is_empty() {
            return Err(RegistrationJobError::Evidence(
                "every control requires a landmark and target reference".into(),
            ));
        }
    }
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
    source.scan_sha256.clone().ok_or_else(|| {
        RegistrationJobError::Evidence(format!("{} has no scan fingerprint", source.id))
    })
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
