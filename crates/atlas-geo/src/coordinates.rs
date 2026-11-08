//! Source coordinate precision and datum are independent pieces of evidence.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::body::BodyId;

/// Angular location on a body, in degrees east and north.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LonLat {
    /// Longitude, -180 through +180 degrees.
    pub longitude: f64,
    /// Latitude, -90 through +90 degrees.
    pub latitude: f64,
}

impl LonLat {
    /// Check numerical range and reject NaN/infinity.
    pub fn validate(self) -> Result<(), CoordinateError> {
        if (-180.0..=180.0).contains(&self.longitude) && (-90.0..=90.0).contains(&self.latitude) {
            Ok(())
        } else {
            Err(CoordinateError::OutOfRange(self))
        }
    }

    /// Convert integral degrees and arcminutes from a northern/eastern table.
    pub fn from_ne_minutes(
        lon_deg: u16,
        lon_min: u8,
        lat_deg: u8,
        lat_min: u8,
    ) -> Result<Self, CoordinateError> {
        if lon_min >= 60 || lat_min >= 60 {
            return Err(CoordinateError::InvalidMinutes);
        }
        let point = Self {
            longitude: f64::from(lon_deg) + f64::from(lon_min) / 60.0,
            latitude: f64::from(lat_deg) + f64::from(lat_min) / 60.0,
        };
        point.validate()?;
        Ok(point)
    }
}

/// Datum explicitly supported by the observation's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Datum {
    /// Source does not specify the datum.
    Unspecified,
    /// Earth WGS84.
    Wgs84,
    /// IAU Moon body-fixed coordinates.
    MoonIau,
    /// IAU Mars body-fixed coordinates.
    MarsIau,
}

impl Datum {
    /// Check whether a datum belongs to the indicated celestial body.
    pub fn validate_body(self, body: BodyId) -> Result<(), CoordinateError> {
        if matches!(
            (body, self),
            (_, Self::Unspecified)
                | (BodyId::Earth, Self::Wgs84)
                | (BodyId::Moon, Self::MoonIau)
                | (BodyId::Mars, Self::MarsIau)
        ) {
            Ok(())
        } else {
            Err(CoordinateError::BodyDatum { body, datum: self })
        }
    }
}

/// A transcribed coordinate rather than a fitted modern location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinateObservation {
    /// Numeric transcription of the angular values.
    pub point: LonLat,
    /// Exact source notation for longitude.
    pub longitude_text: String,
    /// Exact source notation for latitude.
    pub latitude_text: String,
    /// Angular reporting resolution in arcminutes, not a measured error radius.
    pub resolution_arcminutes: f64,
    /// Source datum.
    pub datum: Datum,
}

impl CoordinateObservation {
    /// Validate range, precision, and compatibility with the entity's body.
    pub fn validate(&self, body: BodyId) -> Result<(), CoordinateError> {
        self.point.validate()?;
        if !self.resolution_arcminutes.is_finite() || self.resolution_arcminutes <= 0.0 {
            return Err(CoordinateError::InvalidResolution);
        }
        self.datum.validate_body(body)
    }
}

/// Invalid coordinate evidence.
#[derive(Debug, Error)]
pub enum CoordinateError {
    /// Values exceed the angular domain or are nonfinite.
    #[error("coordinate outside angular domain: {0:?}")]
    OutOfRange(LonLat),
    /// Arcminutes must be below 60.
    #[error("coordinate arcminutes must be below 60")]
    InvalidMinutes,
    /// Reporting precision is invalid.
    #[error("coordinate resolution must be finite and positive")]
    InvalidResolution,
    /// Datum belongs to a different body.
    #[error("datum {datum:?} does not belong to {body:?}")]
    BodyDatum {
        /// Body of the entity.
        body: BodyId,
        /// Datum of the observation.
        datum: Datum,
    },
}

#[cfg(test)]
mod tests;
