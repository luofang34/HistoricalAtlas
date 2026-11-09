use super::*;
use crate::model::{Record, Source};
use atlas_core::{
    assertion::{AdministrativeRank, DemographicsValue, HierarchyValue, NameKind, NameValue},
    calendar::{Calendar, CivilDate},
    time::{InstantEstimate, TemporalSpan, TimePrecision},
};

#[cfg(test)]
fn ledger() -> (SourceCatalog, Dataset) {
    let mut data: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/bijie/page-20.dataset.json"
    ))
    .unwrap();
    let mut history: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/bijie/town-history.dataset.json"
    ))
    .unwrap();
    data.records.append(&mut history.records);
    data.estimates.append(&mut history.estimates);
    data.disputes.append(&mut history.disputes);
    let catalog = SourceCatalog {
        sources: vec![Source {
            capture: None,
            id: "bijie-gazetteer".into(),
            title: "贵州省毕节县地名录".into(),
            responsibility: None,
            publication_year: None,
            reference_year: None,
            scan_pages: Some(660),
            scan_sha256: Some(
                "3591c54d1e4438ce87241df13858159c07135a4fd1efffe87df4aa6e1fd91bb1".into(),
            ),
            url: None,
            rights: None,
        }],
    };
    (catalog, data)
}

#[cfg(test)]
fn date(year: i64, month: u8, day: u8) -> EpochTime {
    CivilDate { year, month, day }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap()
}

#[cfg(test)]
fn names<'a>(slice: &'a TimeSlice<'_>) -> Vec<&'a str> {
    slice
        .entities
        .iter()
        .flat_map(|entity| &entity.properties)
        .flat_map(|property| &property.candidates)
        .filter_map(|candidate| match &candidate.assertion.value {
            TemporalValue::Name(name) => Some(name.text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn source_chronology_changes_names_and_rank_without_promoting_undated_rows() {
    let (catalog, data) = ledger();
    let timeline = Timeline::new(&catalog, &data).unwrap();
    for (year, expected, rank) in [
        (1955, "城关区", AdministrativeRank::District),
        (1960, "灯塔人民公社", AdministrativeRank::Commune),
        (1966, "城关镇", AdministrativeRank::Town),
        (1982, "毕节镇", AdministrativeRank::Town),
    ] {
        let slice = timeline
            .at(date(year, 7, 1), TimeSelection::Nominal)
            .unwrap();
        assert_eq!(names(&slice), [expected]);
        assert_eq!(slice.entities.len(), 1);
        let properties = &slice.entities[0].properties;
        let hierarchy = properties
            .iter()
            .find(|p| p.property == TemporalProperty::Hierarchy)
            .unwrap();
        assert!(matches!(&hierarchy.candidates[0].assertion.value,
            TemporalValue::Hierarchy(value) if value.rank == rank));
        let membership = if year == 1982 {
            TimeMembership::Possible
        } else {
            TimeMembership::Certain
        };
        assert!(
            properties
                .iter()
                .flat_map(|p| &p.candidates)
                .all(|c| c.membership == membership)
        );
        for candidate in properties.iter().flat_map(|p| &p.candidates) {
            assert_eq!(candidate.disputes.len(), usize::from(year == 1955));
            assert!(candidate.basis.iter().all(|id| {
                data.records
                    .iter()
                    .any(|r| r.id == *id && r.checked && r.scan_page == Some(47))
            }));
        }
    }
}

#[test]
fn year_boundary_exposes_competing_values_and_switches_nominally_once() {
    let (catalog, data) = ledger();
    let timeline = Timeline::new(&catalog, &data).unwrap();
    let possible = timeline
        .at(date(1958, 4, 1), TimeSelection::Possible)
        .unwrap();
    let mut candidates = names(&possible);
    candidates.sort();
    assert_eq!(candidates, ["城关区", "灯塔人民公社"]);
    assert!(
        possible.entities[0]
            .properties
            .iter()
            .all(|p| p.conflicting)
    );
    assert!(
        timeline
            .at(date(1958, 4, 1), TimeSelection::Certain)
            .unwrap()
            .entities
            .is_empty()
    );
    let start = data.estimates.iter().find_map(|estimate| match &estimate.value {
        EstimateValue::Temporal { assertion }
            if matches!(&assertion.value, TemporalValue::Name(n) if n.text == "灯塔人民公社") => assertion.validity.start,
        _ => None,
    }).unwrap().nominal;
    assert_eq!(
        names(
            &timeline
                .at(start.checked_add(-1).unwrap(), TimeSelection::Nominal)
                .unwrap()
        ),
        ["城关区"]
    );
    assert_eq!(
        names(&timeline.at(start, TimeSelection::Nominal).unwrap()),
        ["灯塔人民公社"]
    );
}

#[test]
fn property_slots_keep_languages_separate_and_preserve_demographic_quantities() {
    let (catalog, mut data) = ledger();
    let mut translated = data
        .estimates
        .iter()
        .find(|estimate| {
            matches!(&estimate.value,
        EstimateValue::Temporal { assertion } if matches!(&assertion.value, TemporalValue::Name(_)))
        })
        .unwrap()
        .clone();
    translated.key = "test/town-name-en".into();
    translated.id = FactId::from_key(&translated.key).unwrap();
    if let EstimateValue::Temporal { assertion } = &mut translated.value {
        assertion.value = TemporalValue::Name(NameValue {
            text: "Chengguan District".into(),
            kind: NameKind::Standard,
            language: Some("en".into()),
        });
    }
    let mut demographic = translated.clone();
    demographic.key = "test/town-demographics".into();
    demographic.id = FactId::from_key(&demographic.key).unwrap();
    if let EstimateValue::Temporal { assertion } = &mut demographic.value {
        assertion.value = TemporalValue::Demographics(DemographicsValue {
            households: Some(20),
            population: Some(100),
            ethnicities: vec!["汉".into()],
        });
    }
    data.estimates.extend([translated, demographic]);
    let timeline = Timeline::new(&catalog, &data).unwrap();
    let slice = timeline
        .at(date(1955, 1, 1), TimeSelection::Certain)
        .unwrap();
    assert_eq!(slice.entities[0].properties.len(), 4);
    assert!(slice.entities[0].properties.iter().all(|p| !p.conflicting));
    let json = serde_json::to_value(&slice).unwrap();
    let demographic = json["entities"][0]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["property"]["property"] == "demographics")
        .unwrap();
    assert_eq!(
        demographic["candidates"][0]["assertion"]["value"]["value"]["population"],
        100
    );
}

#[cfg(test)]
fn hierarchy(record: &Record, parent: EntityId, start: i128, end: i128) -> Estimate {
    let key = format!("test/hierarchy/{}", record.entity_id.0);
    let exact = |ticks| InstantEstimate {
        earliest: EpochTime::from_ticks(ticks),
        nominal: EpochTime::from_ticks(ticks),
        latest: EpochTime::from_ticks(ticks),
        precision: TimePrecision::Millisecond,
    };
    Estimate {
        id: FactId::from_key(&key).unwrap(),
        key,
        entity_id: record.entity_id,
        basis: vec![record.id],
        brief: "Hierarchy fixture".into(),
        value: EstimateValue::Temporal {
            assertion: TemporalAssertion {
                validity: TemporalSpan {
                    start: Some(exact(start)),
                    end: Some(exact(end)),
                },
                value: TemporalValue::Hierarchy(HierarchyValue {
                    rank: AdministrativeRank::District,
                    parent_id: Some(parent),
                    seat_id: None,
                }),
            },
        },
    }
}

#[test]
fn hierarchy_cycle_requires_simultaneous_links_and_checked_subject_evidence() {
    let (catalog, mut data) = ledger();
    data.disputes.clear();
    data.estimates = vec![
        hierarchy(&data.records[0], data.records[1].entity_id, 0, 10),
        hierarchy(&data.records[1], data.records[0].entity_id, 5, 15),
    ];
    assert!(
        Timeline::new(&catalog, &data)
            .err()
            .unwrap()
            .problem
            .contains("cycle")
    );
    data.estimates[1] = hierarchy(&data.records[1], data.records[0].entity_id, 10, 20);
    assert!(Timeline::new(&catalog, &data).is_ok());
    data.records[1].checked = false;
    assert!(
        Timeline::new(&catalog, &data)
            .err()
            .unwrap()
            .problem
            .contains("checked")
    );
    data.records[1].checked = true;
    data.estimates[1].basis = vec![data.records[0].id];
    assert!(
        Timeline::new(&catalog, &data)
            .err()
            .unwrap()
            .problem
            .contains("subject")
    );
}
