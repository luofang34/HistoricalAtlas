use super::*;

#[test]
fn wuliping_renaming_keeps_year_uncertainty_in_the_actual_timeline() {
    use atlas_core::{
        assertion::TemporalValue,
        calendar::{Calendar, CivilDate},
        identity::EntityId,
    };
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    let id = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    for (year, selection, expected) in [
        (1957, TimeSelection::Certain, vec![]),
        (1960, TimeSelection::Certain, vec!["五里公社"]),
        (1981, TimeSelection::Certain, vec![]),
        (
            1981,
            TimeSelection::Possible,
            vec!["五里公社", "五里坪公社"],
        ),
        (1982, TimeSelection::Nominal, vec!["五里坪公社"]),
        (1982, TimeSelection::Possible, vec!["五里坪公社"]),
        (1982, TimeSelection::Certain, vec![]),
    ] {
        let at = CivilDate {
            year,
            month: 1,
            day: 1,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, selection).unwrap();
        let mut names: Vec<_> = slice
            .entities
            .iter()
            .filter(|entity| entity.entity_id == id)
            .flat_map(|entity| &entity.properties)
            .flat_map(|property| &property.candidates)
            .filter_map(|candidate| {
                if let TemporalValue::Name(name) = &candidate.assertion.value {
                    Some(name.text.as_str())
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        assert_eq!(names, expected, "{year} {selection:?}");
    }
}

#[test]
fn guard_properties_end_in_the_converted_lunar_month() {
    use atlas_core::calendar::{Calendar, CivilDate};
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let guard = input
        .data
        .entities
        .iter()
        .find(|entity| entity.key.ends_with("/bijie-guard"))
        .unwrap();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, month, day, selection, present) in [
        (1678, 7, 1, TimeSelection::Certain, true),
        (1687, 5, 10, TimeSelection::Certain, true),
        (1687, 5, 11, TimeSelection::Certain, false),
        (1687, 5, 11, TimeSelection::Possible, true),
        (1687, 6, 10, TimeSelection::Possible, false),
    ] {
        let at = CivilDate { year, month, day }
            .to_epoch(Calendar::Gregorian, 0)
            .unwrap();
        let slice = timeline.at(at, selection).unwrap();
        let properties = slice
            .entities
            .iter()
            .find(|entity| entity.entity_id == guard.id)
            .map(|entity| entity.properties.len())
            .unwrap_or_default();
        assert_eq!(properties, if present { 2 } else { 0 });
    }
}

#[test]
fn bijie_era_origins_follow_cited_periods() {
    use atlas_core::origin::OriginTime;
    use atlas_ledger::model::EstimateValue;
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for name in [
        "lulu",
        "luzhou",
        "bijie-post-station",
        "bijie-guard",
        "bijie-county",
    ] {
        let estimate = input
            .data
            .estimates
            .iter()
            .find(|estimate| {
                estimate
                    .key
                    .starts_with(&format!("editorial/bijie/{name}/"))
                    && matches!(estimate.value, EstimateValue::Origin { .. })
            })
            .unwrap();
        let record = input
            .data
            .records
            .iter()
            .find(|record| record.id == estimate.basis[0])
            .unwrap();
        let conversion = record
            .source_date
            .as_ref()
            .unwrap()
            .era_conversion
            .as_ref()
            .unwrap();
        let expected = conversion.to_estimate(0).unwrap();
        let EstimateValue::Origin { time } = &estimate.value else {
            panic!("expected origin")
        };
        let (actual, bounded) = match time {
            OriginTime::Estimated { instant } => (instant, true),
            OriginTime::NoLaterThan { limit } => (limit, false),
            _ => panic!("expected bounded or latest-formation date"),
        };
        assert_eq!(bounded, matches!(name, "luzhou" | "bijie-county"));
        assert_eq!(*actual, expected);
        for (at, selection, included) in [
            (
                expected.earliest.checked_add(-1).unwrap(),
                TimeSelection::Possible,
                !bounded,
            ),
            (expected.nominal, TimeSelection::Certain, false),
            (expected.latest, TimeSelection::Certain, true),
        ] {
            let slice = timeline.at(at, selection).unwrap();
            assert_eq!(
                slice
                    .entities
                    .iter()
                    .flat_map(|entity| &entity.origins)
                    .any(|origin| origin.estimate_id == estimate.id),
                included
            );
        }
    }
}

#[test]
fn published_county_evidence_keeps_the_printed_year_and_era_conversion() {
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let county = input
        .data
        .entities
        .iter()
        .find(|entity| entity.key.ends_with("/bijie-county"))
        .unwrap();
    let output = compile_slice(
        &input,
        atlas_core::calendar::CivilDate {
            year: 1688,
            month: 1,
            day: 1,
        }
        .to_epoch(atlas_core::calendar::Calendar::Gregorian, 0)
        .unwrap(),
        TimeSelection::Certain,
    )
    .unwrap();
    let path = std::path::PathBuf::from(format!("entities/index/{}.json", county.id.shard()));
    let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
    let evidence = &shard[county.id.0.to_string()];
    assert_eq!(evidence["disputes"].as_array().unwrap().len(), 1);
    let records = evidence["records"].as_array().unwrap();
    assert!(
        records
            .iter()
            .any(|record| record["source_date"]["text"] == "公元1678年")
    );
    let dated = records
        .iter()
        .find(|record| record["key"] == "bijie-gazetteer/p6/county-kangxi-26-month-4")
        .unwrap();
    assert_eq!(dated["page_or_sheet"], "6");
    assert_eq!(dated["scan_page"], 41);
    assert_eq!(
        dated["source_date"]["era_conversion"]["first_day"]["year"],
        1687
    );
    assert_eq!(dated["source_date"]["era_conversion"]["period"]["month"], 4);
    assert!(
        dated["verbatim_quote"]
            .as_str()
            .unwrap()
            .contains("公元1678年")
    );
}
