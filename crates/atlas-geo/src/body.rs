//! Reference bodies compatible with the Simulate ellipsoid conventions.

use serde::{Deserialize, Serialize};

/// Celestial body namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyId {
    /// Earth.
    Earth,
    /// Earth's Moon.
    Moon,
    /// Mars.
    Mars,
}

/// Reference ellipsoid dimensions, in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyEllipsoid {
    /// Equatorial radius.
    pub semi_major_m: f64,
    /// Flattening `(a-b)/a`.
    pub flattening: f64,
}

impl BodyId {
    /// Earth WGS84, Moon IAU mean sphere, or Mars IAU spheroid.
    pub const fn ellipsoid(self) -> BodyEllipsoid {
        match self {
            Self::Earth => BodyEllipsoid {
                semi_major_m: 6_378_137.0,
                flattening: 1.0 / 298.257_223_563,
            },
            Self::Moon => BodyEllipsoid {
                semi_major_m: 1_737_400.0,
                flattening: 0.0,
            },
            Self::Mars => BodyEllipsoid {
                semi_major_m: 3_396_190.0,
                flattening: 1.0 / 169.894_447_223_611_8,
            },
        }
    }
}
