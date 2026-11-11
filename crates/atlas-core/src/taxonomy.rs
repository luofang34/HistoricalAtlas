//! Domain-grouped entity taxonomy decoupled from time-varying administrative rank.

use serde::{Deserialize, Serialize};

use crate::physical::InfrastructureKind;

/// Celestial and planetary-scale entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanetaryKind {
    /// Celestial body.
    Body,
}

impl PlanetaryKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Body => "body",
        }
    }
}

/// Lithospheric and crustal tectonic structures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TectonicKind {
    /// Rigid or deforming tectonic plate, block, or terrane.
    Plate,
}

impl TectonicKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plate => "plate",
        }
    }
}

/// Geological localities, stratigraphic exposures, and impact structures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeologicalKind {
    /// Geological or paleontological observation locality.
    Site,
    /// Impact crater or multi-ring basin.
    Crater,
}

impl GeologicalKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Crater => "crater",
        }
    }
}

/// Political, military, and administrative jurisdictions.
///
/// Time-varying bureaucratic ranks (such as county, prefecture, guard, commune,
/// town, or brigade) belong to [`crate::assertion::AdministrativeRank`] on source
/// observations and temporal hierarchy assertions rather than permanent identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolityKind {
    /// Sovereign, dynastic, autonomous, or indigenous polity.
    Realm,
    /// Civil or collective territorial division.
    Administrative,
    /// Military territorial command or garrison jurisdiction.
    Military,
}

impl PolityKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Realm => "realm",
            Self::Administrative => "administrative",
            Self::Military => "military",
        }
    }
}

/// Inhabited localities and unclassified named places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceKind {
    /// Settlement or residential locality.
    Settlement,
    /// Named place whose physical or administrative subtype is unstated by the source.
    Toponym,
}

impl PlaceKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Settlement => "settlement",
            Self::Toponym => "toponym",
        }
    }
}

/// Surface and subsurface water features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HydrographyKind {
    /// River, stream, or channel.
    River,
    /// Impounded reservoir.
    Reservoir,
    /// Natural or managed pond.
    Pond,
}

impl HydrographyKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::River => "river",
            Self::Reservoir => "reservoir",
            Self::Pond => "pond",
        }
    }
}

/// Topographic and geomorphological landforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandformKind {
    /// Mountain, peak, or ridge.
    Mountain,
    /// Cave or named underground opening.
    Cave,
}

impl LandformKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mountain => "mountain",
            Self::Cave => "cave",
        }
    }
}

/// Biological and ecological features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EcologyKind {
    /// Forest or managed forest site.
    Forest,
    /// Individually named or recorded tree.
    Tree,
}

impl EcologyKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Forest => "forest",
            Self::Tree => "tree",
        }
    }
}

/// Cultural, historical, and archaeological heritage sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeritageKind {
    /// Historical, commemorative, or archaeological site.
    HistoricSite,
}

impl HeritageKind {
    /// Canonical lowercase subtype identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HistoricSite => "historic_site",
        }
    }
}

/// Domain-grouped geographical entity category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// Celestial body.
    Planetary(PlanetaryKind),
    /// Tectonic plate or crustal block.
    Tectonic(TectonicKind),
    /// Geological locality or impact crater.
    Geology(GeologicalKind),
    /// Political, military, or administrative unit.
    Polity(PolityKind),
    /// Inhabited settlement or unclassified named place.
    Place(PlaceKind),
    /// Watercourse or water body.
    Hydrography(HydrographyKind),
    /// Topographic landform.
    Landform(LandformKind),
    /// Forest or individual tree.
    Ecology(EcologyKind),
    /// Built infrastructure or facility.
    Infrastructure(InfrastructureKind),
    /// Historical or archaeological site.
    Heritage(HeritageKind),
}

impl EntityKind {
    /// Top-level ontological domain identifier.
    pub const fn domain(self) -> &'static str {
        match self {
            Self::Planetary(_) => "planetary",
            Self::Tectonic(_) => "tectonic",
            Self::Geology(_) => "geology",
            Self::Polity(_) => "polity",
            Self::Place(_) => "place",
            Self::Hydrography(_) => "hydrography",
            Self::Landform(_) => "landform",
            Self::Ecology(_) => "ecology",
            Self::Infrastructure(_) => "infrastructure",
            Self::Heritage(_) => "heritage",
        }
    }

    /// Domain-specific subtype identifier.
    pub const fn subkind(self) -> &'static str {
        match self {
            Self::Planetary(kind) => kind.as_str(),
            Self::Tectonic(kind) => kind.as_str(),
            Self::Geology(kind) => kind.as_str(),
            Self::Polity(kind) => kind.as_str(),
            Self::Place(kind) => kind.as_str(),
            Self::Hydrography(kind) => kind.as_str(),
            Self::Landform(kind) => kind.as_str(),
            Self::Ecology(kind) => kind.as_str(),
            Self::Infrastructure(kind) => kind.as_str(),
            Self::Heritage(kind) => kind.as_str(),
        }
    }

    /// Slash-delimited scalar code (`"{domain}/{subkind}"`) for flat tag formats such as MVT.
    pub fn code(self) -> String {
        format!("{}/{}", self.domain(), self.subkind())
    }

    /// Whether the category belongs to the global/regional overview tier by default.
    pub const fn is_overview(self) -> bool {
        matches!(
            self,
            Self::Planetary(_)
                | Self::Tectonic(_)
                | Self::Geology(_)
                | Self::Polity(_)
                | Self::Infrastructure(InfrastructureKind::Airfield | InfrastructureKind::Port)
        )
    }
}

#[cfg(test)]
mod tests;
