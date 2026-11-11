//! Physical measurements, infrastructure categories, and site lifecycle states.

use serde::{Deserialize, Serialize};

/// Quantity measured in metres, retaining the source notation separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementKind {
    /// Height relative to the stated vertical datum.
    Elevation,
    /// End-to-end length.
    Length,
    /// Transverse width.
    Width,
    /// Vertical extent of a structure.
    Height,
    /// Depth below a stated reference surface.
    Depth,
}

/// Cited physical quantity with explicit units and reference surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    /// Quantity being measured.
    pub kind: MeasurementKind,
    /// Numeric value in metres.
    pub metres: f64,
    /// Exact notation from the source.
    pub notation: String,
    /// Vertical datum or other reference surface, if stated.
    pub reference: Option<String>,
}

/// Existence and operational state are independent of an entity's permanent identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Existence {
    /// Physical feature is present.
    Present,
    /// Site remains physically present but is no longer used.
    Abandoned,
    /// Feature has been destroyed.
    Destroyed,
    /// Feature has been deliberately removed.
    Demolished,
}

impl Existence {
    /// Whether this state supports displaying the physical feature.
    pub fn is_present(self) -> bool {
        matches!(self, Self::Present | Self::Abandoned)
    }
}

/// Infrastructure subtypes grow without adding variants to the entity category enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfrastructureKind {
    /// Aviation site containing runways and supporting facilities.
    Airfield,
    /// Prepared aircraft takeoff and landing strip.
    Runway,
    /// Individual building.
    Building,
    /// Defensive structure or enclosure.
    Fortification,
    /// Road or highway.
    Road,
    /// Named urban or rural street, road, or lane.
    Street,
    /// Bridge crossing.
    Bridge,
    /// Railway alignment.
    Railway,
    /// Maritime terminal or port.
    Port,
    /// Industrial or mining production site.
    Factory,
    /// Public-service, educational, administrative, or technical facility.
    Facility,
    /// Public park or designed open space.
    Park,
}

impl InfrastructureKind {
    /// Canonical lowercase identifier used in flat scalar properties and vector tiles.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Airfield => "airfield",
            Self::Runway => "runway",
            Self::Building => "building",
            Self::Fortification => "fortification",
            Self::Road => "road",
            Self::Street => "street",
            Self::Bridge => "bridge",
            Self::Railway => "railway",
            Self::Port => "port",
            Self::Factory => "factory",
            Self::Facility => "facility",
            Self::Park => "park",
        }
    }
}
