//! Evidence records retain the source's precision and wording.

use atlas_core::{
    assertion::{AdministrativeRank, TemporalAssertion, TemporalValue},
    calendar::SourceDate,
    identity::{EntityId, FactId},
    origin::OriginTime,
    time::TemporalSpan,
};
use atlas_geo::{
    body::BodyId,
    coordinates::{CoordinateObservation, Datum, LonLat, UnresolvedCoordinate},
    plates::PlateId,
};
use serde::{Deserialize, Serialize};

/// Bibliographic metadata for an identifiable source edition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Stable bibliographic key.
    pub id: String,
    /// Bibliographic display title.
    pub title: String,
    /// Credited author or issuing body, if verified.
    pub responsibility: Option<String>,
    /// Publication year, if verified.
    pub publication_year: Option<i32>,
    /// Statistical reference year, if stated and uniform.
    pub reference_year: Option<i32>,
    /// Number of pages in the consulted scan, paired with its fingerprint.
    pub scan_pages: Option<u32>,
    /// SHA-256 fingerprint identifying the consulted scan, when used.
    pub scan_sha256: Option<String>,
    /// Public source URL, including a DOI resolver when appropriate.
    pub url: Option<String>,
}

/// Source catalog stored as TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceCatalog {
    /// Consulted editions.
    pub sources: Vec<Source>,
}

/// Geographical entity category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// Celestial body.
    Body,
    /// Tectonic plate or block.
    Plate,
    /// Regional administrative unit.
    Region,
    /// County-level unit.
    County,
    /// Special administrative district.
    SpecialDistrict,
    /// County-subordinate district.
    District,
    /// Town.
    Town,
    /// People's commune.
    Commune,
    /// Township.
    Township,
    /// Urban street administration office.
    StreetOffice,
    /// Named street, road, or lane.
    Street,
    /// Production brigade.
    Brigade,
    /// Settlement or residential locality.
    Settlement,
    /// Named place whose type is not established by the row.
    Toponym,
    /// Industrial site.
    Factory,
    /// Public-service, educational, or technical facility.
    Facility,
    /// Public park.
    Park,
    /// Cave or named underground opening.
    Cave,
    /// Forest or managed forest site.
    Forest,
    /// Bridge.
    Bridge,
    /// River.
    River,
    /// Reservoir.
    Reservoir,
    /// Natural or managed pond.
    Pond,
    /// Mountain.
    Mountain,
    /// Historical site.
    HistoricSite,
    /// Geological observation locality.
    GeologicalSite,
    /// Impact crater or basin.
    Crater,
}

/// Enduring identity; names and administrative relationships belong to evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    /// Permanent UUIDv5 identity.
    pub id: EntityId,
    /// Original semantic key used to mint the identity.
    pub key: String,
    /// Body containing this entity.
    pub body: BodyId,
    /// Geographical category.
    pub kind: EntityKind,
}

/// Former name reported by a source, with dates only when supported.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormerName {
    /// Transcribed name.
    pub name: String,
    /// Stated date expression.
    pub source_date: Option<SourceDate>,
    /// Interpreted validity, if established.
    pub validity: Option<TemporalSpan>,
}

/// A single transcribed table row or separately located source assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// Permanent fact identifier.
    pub id: FactId,
    /// Semantic identity of this source assertion.
    pub key: String,
    /// Subject entity.
    pub entity_id: EntityId,
    /// Bibliographic source identifier.
    pub source_id: String,
    /// Printed page number or sheet label, when the source is paginated.
    pub page_or_sheet: Option<String>,
    /// One-based page in the fingerprinted scan, when used.
    pub scan_page: Option<u32>,
    /// Named row, heading, or location within the page.
    pub locator: String,
    /// Short quotation when needed to interpret an assertion.
    pub verbatim_quote: Option<String>,
    /// Whether this transcription was compared with the cited source.
    pub checked: bool,
    /// Original temporal expression, if the record states one.
    pub source_date: Option<SourceDate>,
    /// Values as recorded by the source.
    pub observation: Observation,
}

/// Values transcribed from a source assertion, retaining their stated conventions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Standard name in the source.
    pub name: String,
    /// Pinyin spelling in the source.
    pub pinyin: Option<String>,
    /// Recorded population, absent for blank cells.
    pub population: Option<u64>,
    /// Recorded household count.
    pub households: Option<u64>,
    /// Source labels for population groups, in source order.
    pub ethnicities: Vec<String>,
    /// Parent indicated by the source's table hierarchy.
    pub parent_id: Option<EntityId>,
    /// Administrative role stated by the source.
    pub rank: Option<AdministrativeRank>,
    /// Seat settlement identified by the source.
    pub seat_id: Option<EntityId>,
    /// Transcribed coordinates, including datum and reporting resolution.
    pub coordinate: Option<CoordinateObservation>,
    /// Printed coordinate notation that has no resolved numeric location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_coordinate: Option<UnresolvedCoordinate>,
    /// Source description of route endpoints, extent, or relative location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent: Option<String>,
    /// Former names supported by the source.
    pub former_names: Vec<FormerName>,
    /// Short transcribed annotation.
    pub note: Option<String>,
}

/// A chosen interpretation with supporting evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Estimate {
    /// Identity in the fact namespace.
    pub id: FactId,
    /// Stable semantic key.
    pub key: String,
    /// Subject of the estimate.
    pub entity_id: EntityId,
    /// Checked source records supporting the interpretation.
    pub basis: Vec<FactId>,
    /// Editorial reasoning shown alongside the estimate.
    pub brief: String,
    /// Interpreted value.
    pub value: EstimateValue,
}

/// Typed interpretation rather than an unstructured replacement of a record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EstimateValue {
    /// Association with a plate in a named reconstruction model.
    Plate {
        /// Stable model identifier, resolved by the compiler's model catalog.
        model_id: String,
        /// Numeric moving-plate identity in that model.
        plate_id: PlateId,
    },
    /// Formation or establishment of an entity.
    Origin {
        /// Date constraint, including one-sided or unquantified uncertainty.
        time: OriginTime,
    },
    /// Interpreted location and its spatial uncertainty.
    Location {
        /// Chosen coordinate.
        point: LonLat,
        /// Coordinate datum.
        datum: Datum,
        /// Documented uncertainty radius, metres.
        uncertainty_m: f64,
    },
    /// Interpreted property during a historical interval.
    Temporal {
        /// Name, hierarchy, or demographics with interval uncertainty.
        assertion: TemporalAssertion<TemporalValue>,
    },
}

/// Conflicting assertions with an explicit chosen estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dispute {
    /// Entity under discussion.
    pub entity_id: EntityId,
    /// Records expressing the competing claims.
    pub alternatives: Vec<FactId>,
    /// Estimate used for the map's main interpretation.
    pub estimate_id: FactId,
    /// Concise explanation for readers.
    pub brief: String,
}

/// One independently reviewable geographical data batch.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dataset {
    /// Enduring identities.
    pub entities: Vec<Entity>,
    /// Source observations.
    pub records: Vec<Record>,
    /// Editorial interpretations.
    pub estimates: Vec<Estimate>,
    /// Explicit disagreements.
    pub disputes: Vec<Dispute>,
}
