//! Capture and archival identity independent of pagination.

use atlas_core::{
    calendar::SourceDate,
    identity::EntityId,
    physical::{Existence, Measurement},
};
use serde::{Deserialize, Serialize};

/// Form of the consulted evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Medium {
    /// Printed publication.
    Print,
    /// Cartographic sheet.
    Map,
    /// Aerial photograph.
    AerialPhotograph,
    /// Photograph taken from the ground.
    Photograph,
    /// Satellite image.
    SatelliteImage,
    /// Archival manuscript or report.
    Manuscript,
    /// Structured digital dataset.
    Dataset,
}

/// Identifiers for locating an original item in a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveReference {
    /// Holding institution.
    pub repository: String,
    /// Collection, accession, or call number.
    pub accession: String,
}

/// Acquisition metadata for an edition, image, or archival item.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    /// SHA-256 fingerprint of an unpaginated image or digital capture.
    pub sha256: Option<String>,
    /// Physical or digital medium.
    pub medium: Option<Medium>,
    /// Source-stated capture date, retaining its notation and precision.
    pub date: Option<SourceDate>,
    /// Flight or sortie identifier.
    pub sortie: Option<String>,
    /// Image frame identifier.
    pub frame: Option<String>,
    /// Archival locator independent of URLs or scan files.
    pub archive: Option<ArchiveReference>,
}

/// Physical assertions attached to the record's citation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalObservation {
    /// Dimensions and elevations as stated in the source.
    #[serde(default)]
    pub measurements: Vec<Measurement>,
    /// Feature's physical presence or removal as observed by the source.
    pub existence: Option<Existence>,
    /// Receiving watercourse identified by the source.
    pub flows_into: Option<EntityId>,
}

#[cfg(test)]
mod tests;
