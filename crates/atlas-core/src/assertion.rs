//! Time-varying geographical properties and their declared validity.

use serde::{Deserialize, Serialize};

use crate::{identity::EntityId, time::TemporalSpan};

/// A property value associated with an uncertainty-aware interval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalAssertion<T> {
    /// Period during which the value applies.
    pub validity: TemporalSpan,
    /// Property value during that period.
    pub value: T,
}

/// Function of a name within a time slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameKind {
    /// Official or standard name.
    Standard,
    /// Romanized spelling.
    Pinyin,
    /// Name used in a local language or dialect.
    Local,
}

/// A name and its linguistic role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameValue {
    /// Name text, retaining the source language.
    pub text: String,
    /// Role of the name.
    pub kind: NameKind,
    /// Language identifier when established.
    pub language: Option<String>,
}

/// Administrative role at a particular time, independent of enduring identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdministrativeRank {
    /// Province.
    Province,
    /// Prefecture or administrative region.
    Prefecture,
    /// County.
    County,
    /// Special administrative district.
    SpecialDistrict,
    /// County-subordinate district.
    District,
    /// People's commune.
    Commune,
    /// Commune management area.
    ManagementArea,
    /// Town.
    Town,
    /// Township.
    Township,
    /// Production brigade.
    Brigade,
    /// Natural settlement.
    Settlement,
    /// Street office.
    StreetOffice,
    /// Historical route.
    Route,
    /// Military guard.
    Guard,
    /// Thousand-household military office.
    ThousandHousehold,
    /// Native official's jurisdiction.
    NativeOffice,
    /// Historical li division.
    Li,
}

/// Administrative placement and seat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchyValue {
    /// Administrative role during this interval.
    pub rank: AdministrativeRank,
    /// Immediate superior unit, when established.
    pub parent_id: Option<EntityId>,
    /// Seat settlement, when established.
    pub seat_id: Option<EntityId>,
}

/// Demographic quantities and source population-group labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemographicsValue {
    /// Household count.
    pub households: Option<u64>,
    /// Population count.
    pub population: Option<u64>,
    /// Population groups, retaining the source labels and order.
    pub ethnicities: Vec<String>,
}

/// Property slot used to group competing time-slice values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "property", rename_all = "snake_case")]
pub enum TemporalProperty {
    /// Name of a specified linguistic role.
    Name {
        /// Linguistic role.
        kind: NameKind,
        /// Language of the name, when established.
        language: Option<String>,
    },
    /// Administrative placement.
    Hierarchy,
    /// Demographic quantities.
    Demographics,
    /// Physical presence or destruction of the feature.
    Existence,
    /// Physical dimension or elevation.
    Measurement {
        /// Quantity being measured.
        kind: crate::physical::MeasurementKind,
    },
}

/// Supported time-varying geographical values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "property",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TemporalValue {
    /// Name in use.
    Name(NameValue),
    /// Administrative placement and seat.
    Hierarchy(HierarchyValue),
    /// Demographic observation.
    Demographics(DemographicsValue),
    /// Physical presence, abandonment, or destruction during the interval.
    Existence(crate::physical::Existence),
    /// A measured physical quantity during the interval.
    Measurement(crate::physical::Measurement),
}

impl TemporalValue {
    /// Property slot occupied by this value.
    pub fn property(&self) -> TemporalProperty {
        match self {
            Self::Name(name) => TemporalProperty::Name {
                kind: name.kind,
                language: name.language.clone(),
            },
            Self::Hierarchy(_) => TemporalProperty::Hierarchy,
            Self::Demographics(_) => TemporalProperty::Demographics,
            Self::Existence(_) => TemporalProperty::Existence,
            Self::Measurement(value) => TemporalProperty::Measurement { kind: value.kind },
        }
    }
}

/// A temporally bounded name.
pub type NameAssertion = TemporalAssertion<NameValue>;
/// A temporally bounded administrative placement.
pub type HierarchyAssertion = TemporalAssertion<HierarchyValue>;
/// A temporally bounded demographic observation.
pub type DemographicsAssertion = TemporalAssertion<DemographicsValue>;
