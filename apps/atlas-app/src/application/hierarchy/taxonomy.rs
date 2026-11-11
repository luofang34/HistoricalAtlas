use atlas_core::{assertion::AdministrativeRank, taxonomy::EntityKind};

/// Top-level ontological domain bucket in browse order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::application) enum DomainGroup {
    Polity,
    Place,
    Hydrography,
    Landform,
    Ecology,
    Infrastructure,
    Heritage,
    Geology,
    Tectonic,
    Planetary,
}

impl DomainGroup {
    pub(super) const fn from_kind(kind: Option<EntityKind>) -> Self {
        match kind {
            Some(EntityKind::Polity(_)) => Self::Polity,
            Some(EntityKind::Place(_)) | None => Self::Place,
            Some(EntityKind::Hydrography(_)) => Self::Hydrography,
            Some(EntityKind::Landform(_)) => Self::Landform,
            Some(EntityKind::Ecology(_)) => Self::Ecology,
            Some(EntityKind::Infrastructure(_)) => Self::Infrastructure,
            Some(EntityKind::Heritage(_)) => Self::Heritage,
            Some(EntityKind::Geology(_)) => Self::Geology,
            Some(EntityKind::Tectonic(_)) => Self::Tectonic,
            Some(EntityKind::Planetary(_)) => Self::Planetary,
        }
    }

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Polity => "Polities & jurisdictions",
            Self::Place => "Settlements & places",
            Self::Hydrography => "Hydrography",
            Self::Landform => "Landforms",
            Self::Ecology => "Ecology",
            Self::Infrastructure => "Infrastructure",
            Self::Heritage => "Heritage sites",
            Self::Geology => "Geology",
            Self::Tectonic => "Tectonic plates",
            Self::Planetary => "Planetary bodies",
        }
    }
}

pub(in crate::application) const fn rank_label(rank: AdministrativeRank) -> &'static str {
    match rank {
        AdministrativeRank::Province => "province",
        AdministrativeRank::Prefecture => "prefecture",
        AdministrativeRank::County => "county",
        AdministrativeRank::SpecialDistrict => "special district",
        AdministrativeRank::District => "district",
        AdministrativeRank::Town => "town",
        AdministrativeRank::StreetOffice => "street office",
        AdministrativeRank::Commune => "commune",
        AdministrativeRank::Township => "township",
        AdministrativeRank::ManagementArea => "management area",
        AdministrativeRank::Brigade => "brigade",
        AdministrativeRank::Route => "route",
        AdministrativeRank::Guard => "guard",
        AdministrativeRank::ThousandHousehold => "thousand-household",
        AdministrativeRank::NativeOffice => "native office",
        AdministrativeRank::Li => "li",
        AdministrativeRank::Settlement => "settlement",
    }
}

pub(super) const fn rank_order(rank: AdministrativeRank) -> u8 {
    match rank {
        AdministrativeRank::Province => 1,
        AdministrativeRank::Prefecture => 2,
        AdministrativeRank::SpecialDistrict => 3,
        AdministrativeRank::County => 4,
        AdministrativeRank::Route => 5,
        AdministrativeRank::Guard => 6,
        AdministrativeRank::ThousandHousehold => 7,
        AdministrativeRank::NativeOffice => 8,
        AdministrativeRank::District => 9,
        AdministrativeRank::Town => 10,
        AdministrativeRank::StreetOffice => 11,
        AdministrativeRank::Commune => 12,
        AdministrativeRank::Township => 13,
        AdministrativeRank::Li => 14,
        AdministrativeRank::ManagementArea => 15,
        AdministrativeRank::Brigade => 16,
        AdministrativeRank::Settlement => 17,
    }
}

pub(super) fn entity_badge(kind: Option<EntityKind>, ranks: &[AdministrativeRank]) -> String {
    if let Some(&rank) = ranks.iter().min_by_key(|&&r| rank_order(r)) {
        return rank_label(rank).to_owned();
    }
    kind.map_or_else(String::new, |k| k.subkind().replace('_', " "))
}

pub(super) fn category_subkind(kind: Option<EntityKind>, ranks: &[AdministrativeRank]) -> String {
    if matches!(kind, Some(EntityKind::Polity(_)))
        && let Some(&rank) = ranks.iter().min_by_key(|&&r| rank_order(r))
    {
        return rank_label(rank).to_owned();
    }
    kind.map_or_else(|| "place".to_owned(), |k| k.subkind().replace('_', " "))
}
