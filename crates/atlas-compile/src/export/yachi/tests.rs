use super::*;
use crate::input::{LedgerInput, load_blocking};
use atlas_ledger::model::EntityKind;

#[cfg(test)]
fn input() -> LedgerInput {
    load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

#[cfg(test)]
fn id(key: &str) -> EntityId {
    EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap()
}

#[cfg(test)]
fn table<'a>(input: &'a LedgerInput, key: &str) -> &'a Record {
    input
        .data
        .records
        .iter()
        .find(|record| {
            record.entity_id == id(key)
                && record.source_id == "bijie-gazetteer"
                && matches!(record.scan_page, Some(79 | 81 | 84 | 85 | 87))
        })
        .unwrap()
}

#[test]
fn reviewed_tables_publish_every_row_with_original_precision_and_unknown_counts() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [(44, 33), (46, 29), (49, 36), (50, 22), (52, 29)] {
        let records: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|record| {
                record.source_id == "bijie-gazetteer" && record.scan_page == Some(page + 35)
            })
            .collect();
        assert_eq!(records.len(), count);
        for record in records {
            assert!(record.checked);
            assert!(record.source_date.is_none());
            let path = PathBuf::from(format!("entities/index/{}.json", record.entity_id.shard()));
            let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
            let published = shard[record.entity_id.0.to_string()]["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|value| value["id"] == record.id.0.to_string())
                .unwrap();
            assert_eq!(published["page_or_sheet"], page.to_string());
            assert_eq!(published["scan_page"], page + 35);
            assert_eq!(published, &serde_json::to_value(record).unwrap());
            if let Some(coordinate) = &record.observation.coordinate {
                assert_eq!(coordinate.datum, atlas_geo::coordinates::Datum::Unspecified);
                assert_eq!(coordinate.resolution_arcminutes, 1.0);
            }
        }
    }
}

#[test]
fn brigade_sections_and_seats_survive_page_continuations_and_explicit_exceptions() {
    let input = input();
    for (commune, brigades) in [("yachi", 5), ("yihe", 4), ("nanqing", 8), ("baohe", 4)] {
        let parent = id(&format!("{commune}-commune"));
        let children: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|record| {
                record.observation.parent_id == Some(parent)
                    && record.observation.rank
                        == Some(atlas_core::assertion::AdministrativeRank::Brigade)
            })
            .collect();
        assert_eq!(children.len(), brigades);
        assert!(
            children
                .iter()
                .all(|record| record.observation.seat_id.is_some())
        );
    }
    assert_eq!(
        table(&input, "nanqing/liujiadong").observation.parent_id,
        Some(id("nanqing/hetao-brigade"))
    );
    assert_eq!(
        table(&input, "nanqing/zhongzhai").observation.parent_id,
        Some(id("nanqing/meichong-brigade"))
    );
    assert_eq!(
        table(&input, "baohe/shangxiaohe-brigade")
            .observation
            .seat_id,
        Some(id("baohe/xiaxiaohe"))
    );
}

#[test]
fn homonyms_keep_their_row_identities_types_and_unresolved_correspondence() {
    let input = input();
    let first = table(&input, "yachi/zhongzhai-yachi");
    let second = table(&input, "yachi/zhongzhai-shige");
    assert_eq!(first.observation.name, second.observation.name);
    assert_ne!(first.entity_id, second.entity_id);
    assert_ne!(first.observation.parent_id, second.observation.parent_id);
    assert_eq!(first.observation.population, Some(134));
    assert!(second.observation.population.is_none());
    let village = table(&input, "nanqing/panzhu-si-village");
    let temple = table(&input, "nanqing/panzhu-si-temple");
    assert_eq!(village.observation.name, temple.observation.name);
    assert_ne!(
        village.observation.coordinate,
        temple.observation.coordinate
    );
    for (record, kind) in [
        (village, EntityKind::Settlement),
        (temple, EntityKind::HistoricSite),
    ] {
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|e| e.id == record.entity_id)
                .unwrap()
                .kind,
            kind
        );
    }
    let first = table(&input, "baohe/zitong-ge-first-entry");
    let second = table(&input, "baohe/zitong-ge-second-entry");
    assert_ne!(first.entity_id, second.entity_id);
    assert_ne!(first.id, second.id);
    assert_eq!(first.observation.name, second.observation.name);
    assert_eq!(
        first.observation.coordinate.as_ref().unwrap().point,
        second.observation.coordinate.as_ref().unwrap().point
    );
    assert!(first.locator.contains("unresolved"));
    assert!(second.locator.contains("unresolved"));
}

#[test]
fn printed_anomalies_and_missing_values_are_not_replaced_by_neighboring_rows() {
    let input = input();
    for (key, latitude) in [
        ("yachi/miaozhai", "24°15′"),
        ("nanqing/ganlongtan", "17°13′"),
    ] {
        assert_eq!(
            table(&input, key)
                .observation
                .coordinate
                .as_ref()
                .unwrap()
                .latitude_text,
            latitude
        );
    }
    assert_eq!(
        table(&input, "nanqing/yantou-brigade")
            .observation
            .population,
        Some(13328)
    );
    let unlocated = table(&input, "nanqing/xianshan");
    assert!(unlocated.observation.coordinate.is_none());
    assert!(unlocated.observation.population.is_none());
    assert!(unlocated.observation.ethnicities.is_empty());
    let unresolved = table(&input, "nanqing/tudiya");
    assert!(unresolved.observation.coordinate.is_none());
    assert_eq!(
        unresolved
            .observation
            .unresolved_coordinate
            .as_ref()
            .unwrap()
            .latitude_text,
        "27′11′"
    );
    assert_eq!(
        table(&input, "yihe/heba-brigade").observation.ethnicities,
        ["汉", "苗", "南京"]
    );
    assert_eq!(
        table(&input, "yachi-commune").observation.ethnicities,
        ["汉", "彝", "苗", "汉"]
    );
    assert!(
        table(&input, "nanqing-commune")
            .observation
            .pinyin
            .is_none()
    );
}

#[test]
fn overviews_and_tables_retain_different_population_and_river_length_claims() {
    let input = input();
    let commune = id("nanqing-commune");
    let population: BTreeSet<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.entity_id == commune)
        .filter_map(|record| record.observation.population)
        .collect();
    assert_eq!(population, BTreeSet::from([10869, 11187]));
    let river = id("nanqing/nanqing-he");
    let descriptions: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.entity_id == river)
        .collect();
    assert_eq!(descriptions.len(), 2);
    for record in &descriptions {
        assert!(record.observation.coordinate.is_none());
        assert!(record.observation.population.is_none());
    }
    assert!(descriptions.iter().any(|record| {
        record
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约5公里")
    }));
    assert!(descriptions.iter().any(|record| {
        record
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约6公里")
    }));
    let pond = input
        .data
        .entities
        .iter()
        .find(|entity| entity.id == id("yachi/gujia-tang"))
        .unwrap();
    assert_eq!(pond.kind, EntityKind::Pond);
    assert!(input.data.records.iter().any(|record| {
        record.entity_id == pond.id
            && record
                .observation
                .extent
                .as_ref()
                .is_some_and(|extent| extent.contains("天然水塘"))
    }));
}

#[test]
fn district_renaming_and_commune_origins_follow_year_uncertainty() {
    use atlas_core::calendar::{Calendar, CivilDate};
    use atlas_ledger::timeline::{TimeSelection, Timeline};
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, expected) in [
        (1953, vec!["二区"]),
        (1954, vec!["二区", "鸭池区"]),
        (1955, vec!["鸭池区"]),
    ] {
        let at = CivilDate {
            year,
            month: 7,
            day: 1,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, TimeSelection::Possible).unwrap();
        let mut names: Vec<_> = slice
            .entities
            .iter()
            .filter(|entity| entity.entity_id == id("yachi-district"))
            .flat_map(|entity| &entity.properties)
            .flat_map(|property| &property.candidates)
            .filter_map(|candidate| match &candidate.assertion.value {
                TemporalValue::Name(name) => Some(name.text.as_str()),
                _ => None,
            })
            .collect();
        names.sort();
        assert_eq!(names, expected);
    }
    for (year, formed) in [(1957, false), (1959, true)] {
        let at = CivilDate {
            year,
            month: 1,
            day: 1,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, TimeSelection::Certain).unwrap();
        for name in ["yachi", "yihe", "nanqing", "baohe"] {
            let origins = slice
                .entities
                .iter()
                .find(|entity| entity.entity_id == id(&format!("{name}-commune")))
                .map(|entity| entity.origins.len())
                .unwrap_or_default();
            assert_eq!(origins, usize::from(formed));
        }
    }
}
