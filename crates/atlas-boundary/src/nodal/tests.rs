use super::*;
use atlas_core::{
    assertion::{AdministrativeRank, HierarchyValue, TemporalAssertion, TemporalValue},
    time::{InstantEstimate, TimePrecision},
};
use atlas_geo::coordinates::{Datum, LonLat, UnresolvedCoordinate};
use atlas_ledger::model::{
    Entity, EntityKind, Estimate, EstimateValue, Observation, Record, Source,
};

#[cfg(test)]
fn id(name: &str) -> EntityId {
    EntityId::from_key(&format!("earth/test/{name}")).unwrap()
}

#[cfg(test)]
fn record(name: &str, parent: Option<&str>, longitude: Option<f64>) -> Record {
    let key = format!("test-source/{name}");
    Record {
        id: FactId::from_key(&key).unwrap(),
        key,
        entity_id: id(name),
        source_id: "test-source".into(),
        page_or_sheet: Some("1".into()),
        scan_page: Some(1),
        locator: name.into(),
        verbatim_quote: None,
        checked: true,
        source_date: None,
        observation: Observation {
            name: name.into(),
            pinyin: None,
            population: None,
            households: None,
            ethnicities: vec![],
            parent_id: parent.map(id),
            rank: None,
            seat_id: None,
            coordinate: longitude.map(|longitude| CoordinateObservation {
                point: LonLat {
                    longitude,
                    latitude: 27.0,
                },
                datum: Datum::Unspecified,
                longitude_text: longitude.to_string(),
                latitude_text: "27°".into(),
                resolution_arcminutes: 1.0,
            }),
            unresolved_coordinate: None,
            extent: None,
            territory: None,
            former_names: vec![],
            note: None,
        },
    }
}

#[cfg(test)]
fn fixture() -> (SourceCatalog, Dataset) {
    let catalog = SourceCatalog {
        sources: vec![Source {
            id: "test-source".into(),
            title: "Test source".into(),
            responsibility: None,
            publication_year: None,
            reference_year: None,
            scan_pages: Some(1),
            scan_sha256: Some("0".repeat(64)),
            url: None,
        }],
    };
    let entities = [
        "root",
        "seat",
        "brigade",
        "village",
        "coincident",
        "unlocated",
    ]
    .into_iter()
    .map(|name| Entity {
        id: id(name),
        key: format!("earth/test/{name}"),
        body: BodyId::Earth,
        kind: EntityKind::Settlement,
    })
    .collect();
    let mut root = record("root", None, None);
    root.observation.seat_id = Some(id("seat"));
    let mut unlocated = record("unlocated", Some("brigade"), None);
    unlocated.observation.unresolved_coordinate = Some(UnresolvedCoordinate {
        longitude_text: "185°".into(),
        latitude_text: "27°".into(),
        resolution_arcminutes: 1.0,
        datum: Datum::Unspecified,
        reason: "Longitude outside angular domain.".into(),
    });
    let records = vec![
        root,
        record("seat", Some("root"), Some(105.0)),
        record("brigade", Some("root"), Some(106.0)),
        record("village", Some("brigade"), Some(107.0)),
        record("coincident", Some("brigade"), Some(107.0)),
        unlocated,
    ];
    (
        catalog,
        Dataset {
            entities,
            records,
            ..Default::default()
        },
    )
}

#[cfg(test)]
fn query(scope: NodalScope) -> NodalQuery {
    NodalQuery {
        entity_id: id("root"),
        scope,
    }
}

#[cfg(test)]
fn source_scope() -> NodalScope {
    NodalScope::Source {
        source_id: "test-source".into(),
    }
}

#[test]
fn network_uses_seat_anchors_and_retains_unlocated_and_coincident_members() {
    let (catalog, data) = fixture();
    let territory = NodalBuilder::new(&catalog, &data)
        .unwrap()
        .build(query(source_scope()))
        .unwrap();
    assert_eq!(territory.nodes.len(), 6);
    assert_eq!(territory.links.len(), 6);
    let missing = territory
        .nodes
        .iter()
        .find(|node| node.entity_id == id("unlocated"))
        .unwrap();
    assert!(missing.anchors.is_empty());
    assert_eq!(
        missing.unresolved_coordinate_records,
        vec![data.records[5].id]
    );
    let geojson = territory.geojson();
    let features = geojson["features"].as_array().unwrap();
    let root = features
        .iter()
        .find(|feature| feature["properties"]["entity_id"] == id("root").0.to_string())
        .unwrap();
    assert_eq!(
        root["geometry"]["coordinates"],
        serde_json::json!([105.0, 27.0])
    );
    assert_eq!(
        root["properties"]["anchor_entity_id"],
        id("seat").0.to_string()
    );
    assert_eq!(
        root["properties"]["seat_evidence"]["record_id"],
        data.records[0].id.0.to_string()
    );
    let points: Vec<_> = features
        .iter()
        .filter(|feature| feature["geometry"]["coordinates"] == serde_json::json!([107.0, 27.0]))
        .collect();
    assert_eq!(points.len(), 2);
    assert_ne!(
        points[0]["properties"]["entity_id"],
        points[1]["properties"]["entity_id"]
    );
    assert!(
        !features
            .iter()
            .any(|feature| feature["properties"]["entity_id"] == id("unlocated").0.to_string())
    );
    let link = features
        .iter()
        .find(|feature| feature["properties"]["to"] == id("brigade").0.to_string())
        .unwrap();
    assert_eq!(
        link["properties"]["from_anchor_entity"],
        id("seat").0.to_string()
    );
    assert_eq!(
        link["properties"]["evidence"]["record_id"],
        data.records[2].id.0.to_string()
    );
    assert_eq!(
        link["properties"]["from_anchor"]["coordinate"]["datum"],
        "unspecified"
    );
}

#[test]
fn source_selection_excludes_other_editions_and_unchecked_relations() {
    let (mut catalog, mut data) = fixture();
    let mut other_source = catalog.sources[0].clone();
    other_source.id = "other-edition".into();
    catalog.sources.push(other_source);
    data.records[3].source_id = "other-edition".into();
    data.records[4].checked = false;
    let territory = NodalBuilder::new(&catalog, &data)
        .unwrap()
        .build(query(source_scope()))
        .unwrap();
    assert!(
        !territory
            .nodes
            .iter()
            .any(|node| [id("village"), id("coincident")].contains(&node.entity_id))
    );
    assert_eq!(territory.nodes.len(), 4);
    let mut reversed = data.clone();
    reversed.records.reverse();
    reversed.entities.reverse();
    let again = NodalBuilder::new(&catalog, &reversed)
        .unwrap()
        .build(query(source_scope()))
        .unwrap();
    assert_eq!(
        serde_json::to_value(territory).unwrap(),
        serde_json::to_value(again).unwrap()
    );
}

#[cfg(test)]
fn hierarchy(data: &Dataset, subject: &str, parent: &str, key: &str, start: [i128; 3]) -> Estimate {
    let instant = |[earliest, nominal, latest]: [i128; 3]| InstantEstimate {
        earliest: EpochTime::from_ticks(earliest),
        nominal: EpochTime::from_ticks(nominal),
        latest: EpochTime::from_ticks(latest),
        precision: TimePrecision::Millisecond,
    };
    Estimate {
        id: FactId::from_key(key).unwrap(),
        key: key.into(),
        entity_id: id(subject),
        basis: vec![
            data.records
                .iter()
                .find(|record| record.entity_id == id(subject))
                .unwrap()
                .id,
        ],
        brief: "Test interval hierarchy.".into(),
        value: EstimateValue::Temporal {
            assertion: TemporalAssertion {
                validity: TemporalSpan {
                    start: Some(instant(start)),
                    end: Some(instant([100, 100, 100])),
                },
                value: TemporalValue::Hierarchy(HierarchyValue {
                    rank: AdministrativeRank::Brigade,
                    parent_id: Some(id(parent)),
                    seat_id: None,
                }),
            },
        },
    }
}

#[test]
fn time_selection_uses_hierarchy_intervals_without_importing_undated_descendants() {
    let (catalog, mut data) = fixture();
    data.estimates.push(hierarchy(
        &data,
        "brigade",
        "root",
        "test/hierarchy",
        [0, 10, 20],
    ));
    for (ticks, selection, count) in [
        (-1, TimeSelection::Possible, 0),
        (5, TimeSelection::Nominal, 0),
        (5, TimeSelection::Possible, 1),
        (5, TimeSelection::Certain, 0),
        (25, TimeSelection::Certain, 1),
        (100, TimeSelection::Possible, 0),
    ] {
        let scope = NodalScope::Time {
            at: EpochTime::from_ticks(ticks),
            selection,
        };
        let territory = NodalBuilder::new(&catalog, &data)
            .unwrap()
            .build(query(scope))
            .unwrap();
        assert_eq!(territory.links.len(), count, "{ticks} {selection:?}");
        assert_eq!(territory.nodes.len(), count * 2);
        if count > 0 {
            assert_eq!(territory.links[0].to, id("brigade"));
            assert!(matches!(
                territory.links[0].evidence,
                RelationEvidence::Estimate { .. }
            ));
            let root = territory
                .nodes
                .iter()
                .find(|node| node.entity_id == id("root"))
                .unwrap();
            assert!(root.anchors.is_empty());
            assert_eq!(territory.geojson()["features"].as_array().unwrap().len(), 1);
        }
    }
}

#[test]
fn alternative_parents_and_cyclic_source_claims_are_retained_without_unbounded_traversal() {
    let (catalog, mut data) = fixture();
    let mut alternative = data.records[3].clone();
    alternative.key = "test-source/alternative".into();
    alternative.id = FactId::from_key(&alternative.key).unwrap();
    alternative.observation.parent_id = Some(id("root"));
    data.records.push(alternative);
    data.records[0].observation.parent_id = Some(id("village"));
    let territory = NodalBuilder::new(&catalog, &data)
        .unwrap()
        .build(query(source_scope()))
        .unwrap();
    assert_eq!(territory.nodes.len(), 6);
    assert_eq!(territory.links.len(), 8);
    assert_eq!(
        territory
            .links
            .iter()
            .filter(|link| link.to == id("village"))
            .count(),
        2
    );
}

#[test]
fn incompatible_datums_do_not_create_coordinate_chords() {
    let (catalog, mut data) = fixture();
    data.records[2]
        .observation
        .coordinate
        .as_mut()
        .unwrap()
        .datum = Datum::Wgs84;
    let territory = NodalBuilder::new(&catalog, &data)
        .unwrap()
        .build(query(source_scope()))
        .unwrap();
    let geojson = territory.geojson();
    assert!(
        !geojson["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|feature| feature["geometry"]["type"] == "MultiLineString")
    );
    assert_eq!(territory.links.len(), 6);
}

#[test]
fn invalid_query_and_cross_body_hierarchy_are_rejected() {
    let (catalog, mut data) = fixture();
    let builder = NodalBuilder::new(&catalog, &data).unwrap();
    assert!(matches!(
        builder.build(NodalQuery {
            entity_id: id("missing"),
            scope: source_scope()
        }),
        Err(NodalError::UnknownEntity(_))
    ));
    assert!(matches!(
        builder.build(query(NodalScope::Source {
            source_id: "missing".into()
        })),
        Err(NodalError::UnknownSource(_))
    ));
    data.entities[0].body = BodyId::Moon;
    assert!(NodalBuilder::new(&catalog, &data).is_err());
}
