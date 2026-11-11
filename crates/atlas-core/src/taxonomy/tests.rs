use super::*;
use serde_json::json;

#[test]
fn hierarchical_entity_kinds_round_trip_and_expose_scalar_codes() {
    let cases = [
        (
            EntityKind::Planetary(PlanetaryKind::Body),
            json!({"planetary": "body"}),
            "planetary/body",
            true,
        ),
        (
            EntityKind::Tectonic(TectonicKind::Plate),
            json!({"tectonic": "plate"}),
            "tectonic/plate",
            true,
        ),
        (
            EntityKind::Geology(GeologicalKind::Crater),
            json!({"geology": "crater"}),
            "geology/crater",
            true,
        ),
        (
            EntityKind::Polity(PolityKind::Administrative),
            json!({"polity": "administrative"}),
            "polity/administrative",
            true,
        ),
        (
            EntityKind::Place(PlaceKind::Settlement),
            json!({"place": "settlement"}),
            "place/settlement",
            false,
        ),
        (
            EntityKind::Hydrography(HydrographyKind::River),
            json!({"hydrography": "river"}),
            "hydrography/river",
            false,
        ),
        (
            EntityKind::Infrastructure(InfrastructureKind::Runway),
            json!({"infrastructure": "runway"}),
            "infrastructure/runway",
            false,
        ),
        (
            EntityKind::Heritage(HeritageKind::HistoricSite),
            json!({"heritage": "historic_site"}),
            "heritage/historic_site",
            false,
        ),
    ];
    for (kind, expected_json, expected_code, overview) in cases {
        let encoded = serde_json::to_value(kind).unwrap();
        assert_eq!(encoded, expected_json);
        let decoded: EntityKind = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, kind);
        assert_eq!(kind.code(), expected_code);
        assert_eq!(kind.is_overview(), overview);
    }
}
