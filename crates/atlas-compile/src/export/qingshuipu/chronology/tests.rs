use super::*;
use atlas_core::{
    assertion::TemporalValue,
    calendar::{Calendar, CivilDate},
};
use atlas_ledger::timeline::{TimeSelection, Timeline};

#[cfg(test)]
fn names(
    input: &LedgerInput,
    subject: &str,
    year: i64,
    selection: TimeSelection,
) -> BTreeSet<String> {
    let at = CivilDate {
        year,
        month: 8,
        day: 15,
    }
    .to_epoch(Calendar::Gregorian, 0)
    .unwrap();
    Timeline::new(&input.catalog, &input.data)
        .unwrap()
        .at(at, selection)
        .unwrap()
        .entities
        .iter()
        .filter(|e| e.entity_id == id(subject))
        .flat_map(|e| &e.properties)
        .flat_map(|p| &p.candidates)
        .filter_map(|c| match &c.assertion.value {
            TemporalValue::Name(n) => Some(n.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn district_commune_and_merger_names_follow_source_year_intervals() {
    let input = input();
    for (subject, year, name) in [
        ("qingshuipu-district", 1955, "第十区"),
        ("qingshuipu-district", 1957, "清水区"),
        ("qingshuipu-commune", 1960, "清水人民公社"),
        ("gaoshanpu-commune", 1960, "高山人民公社"),
    ] {
        assert_eq!(
            names(&input, subject, year, TimeSelection::Certain),
            BTreeSet::from([name.to_owned()])
        );
    }
    for (subject, name) in [
        ("qingshuipu-district", "清水铺区"),
        ("qingshuipu-commune", "清水铺公社"),
        ("gaoshanpu-commune", "高山铺人民公社"),
    ] {
        assert_eq!(
            names(&input, subject, 1982, TimeSelection::Possible),
            BTreeSet::from([name.to_owned()])
        );
        assert!(names(&input, subject, 1982, TimeSelection::Certain).is_empty());
    }
    for (slug, name) in [("nanguan", "南关公社"), ("xiaogou", "小沟公社")] {
        let key = format!("gaoshanpu/{slug}-former-commune");
        assert_eq!(
            names(&input, &key, 1965, TimeSelection::Possible),
            BTreeSet::from([name.to_owned()])
        );
        assert!(names(&input, &key, 1967, TimeSelection::Possible).is_empty());
        assert_ne!(id(&key), id(&format!("gaoshanpu/{slug}-brigade")));
    }
    let citrus = record(
        &input,
        "p306/gaoshanpu/nanguan/chengmanyuan/citrus-introduction",
    );
    assert_eq!(citrus.source_date.as_ref().unwrap().text, "一九六六年后");
    assert!(!input.data.estimates.iter().any(
        |e| e.entity_id == citrus.entity_id && matches!(e.value, EstimateValue::Origin { .. })
    ));
}

#[test]
fn kangxi_claims_retain_both_printed_ce_years_and_the_lunar_month_estimate() {
    let input = input();
    let dispute = input
        .data
        .disputes
        .iter()
        .find(|d| d.entity_id == id("bijie-county"))
        .unwrap();
    assert_eq!(dispute.alternatives.len(), 7);
    for page in [299, 305] {
        let era = record(&input, &format!("p{page}/bijie-county/chishui-merger"));
        let ce = record(
            &input,
            &format!("p{page}/bijie-county/chishui-merger-printed-year"),
        );
        assert!(dispute.alternatives.contains(&era.id));
        assert!(dispute.alternatives.contains(&ce.id));
        assert_eq!(ce.source_date.as_ref().unwrap().text, "公元1686年");
        let conversion = era
            .source_date
            .as_ref()
            .unwrap()
            .era_conversion
            .as_ref()
            .unwrap();
        assert_eq!(
            conversion.first_day,
            CivilDate {
                year: 1687,
                month: 2,
                day: 12
            }
        );
        assert_eq!(
            conversion.last_day,
            CivilDate {
                year: 1688,
                month: 2,
                day: 1
            }
        );
    }
    assert!(input.data.records.iter().any(|r| {
        dispute.alternatives.contains(&r.id)
            && r.source_date
                .as_ref()
                .is_some_and(|d| d.text == "公元1678年")
    }));
    let selected = input
        .data
        .estimates
        .iter()
        .find(|e| e.id == dispute.estimate_id)
        .unwrap();
    assert!(
        selected
            .basis
            .contains(&record(&input, "p6/county-kangxi-26-month-4").id)
    );
}
