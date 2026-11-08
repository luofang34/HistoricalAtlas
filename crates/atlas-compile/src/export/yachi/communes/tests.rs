use super::*;
use crate::input::{LedgerInput, load_blocking};
use atlas_core::calendar::{Calendar, CivilDate};
use atlas_ledger::model::EntityKind;
use atlas_ledger::timeline::{TimeSelection, Timeline};

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
                && matches!(
                    record.scan_page,
                    Some(89 | 90 | 93 | 95 | 97 | 99 | 100 | 102)
                )
        })
        .unwrap()
}

#[cfg(test)]
fn instant(year: i64) -> atlas_core::time::EpochTime {
    CivilDate {
        year,
        month: 7,
        day: 1,
    }
    .to_epoch(Calendar::Gregorian, 0)
    .unwrap()
}

#[test]
fn every_reviewed_commune_table_row_round_trips_through_publication() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [
        (54, 36),
        (55, 3),
        (58, 27),
        (60, 28),
        (62, 26),
        (64, 34),
        (65, 2),
        (67, 27),
    ] {
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
            assert_eq!(record.page_or_sheet, Some(page.to_string()));
            assert!(record.source_date.is_none());
            let path = PathBuf::from(format!("entities/index/{}.json", record.entity_id.shard()));
            let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
            let published = shard[record.entity_id.0.to_string()]["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|value| value["id"] == record.id.0.to_string())
                .unwrap();
            assert_eq!(published, &serde_json::to_value(record).unwrap());
            if let Some(coordinate) = &record.observation.coordinate {
                assert_eq!(coordinate.resolution_arcminutes, 1.0);
                assert_eq!(coordinate.datum, atlas_geo::coordinates::Datum::Unspecified);
            }
        }
    }
}

#[test]
fn all_ten_yachi_communes_have_reviewed_brigades_and_cited_seats() {
    let input = input();
    let mut total = 0;
    for (name, count) in [
        ("yachi", 5),
        ("yihe", 4),
        ("nanqing", 8),
        ("baohe", 4),
        ("shiba", 3),
        ("caodi", 2),
        ("ganhegou", 4),
        ("songlin", 2),
        ("lishu", 3),
        ("caoping", 4),
    ] {
        let parent = id(&format!("{name}-commune"));
        let brigades: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|record| {
                record.observation.parent_id == Some(parent)
                    && record.observation.rank
                        == Some(atlas_core::assertion::AdministrativeRank::Brigade)
            })
            .collect();
        assert_eq!(brigades.len(), count);
        assert!(
            brigades
                .iter()
                .all(|record| record.observation.seat_id.is_some())
        );
        total += brigades.len();
    }
    assert_eq!(total, 39);
    for (unit, seat) in [
        ("shiba-commune", "shiba/shiba"),
        ("caodi-commune", "caodi/ganjia-qiao"),
        ("ganhegou/liaojiapo-brigade", "ganhegou/liaojiaba"),
        ("songlin/lianhe-brigade", "songlin/mawozhai"),
        ("lishu/shuiniutun-brigade", "lishu/jiali"),
        ("caoping-commune", "caoping/yingjiao"),
        ("caoping/zuna-brigade", "caoping/zuna"),
    ] {
        assert_eq!(table(&input, unit).observation.seat_id, Some(id(seat)));
    }
    assert_eq!(
        table(&input, "lishu/sunjiawanzi").observation.parent_id,
        Some(id("lishu/shuiniutun-brigade"))
    );
}

#[test]
fn anomalous_values_and_conflicting_overview_claims_remain_source_specific() {
    let input = input();
    let coordinate = table(&input, "caoping/shalang-brigade")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(coordinate.latitude_text, "72°12′");
    assert_eq!(coordinate.point.latitude, 72.2);
    assert_eq!(
        table(&input, "lishu/erpu-brigade").observation.population,
        Some(150)
    );
    assert_eq!(
        table(&input, "caoping-commune").observation.ethnicities,
        ["彝", "汉", "苗", "南"]
    );
    assert_eq!(
        table(&input, "songlin/sanjiazhai").observation.ethnicities,
        ["南京"]
    );
    assert!(
        table(&input, "songlin/longdong-ying")
            .observation
            .pinyin
            .is_none()
    );
    assert!(
        table(&input, "songlin/longdong-ying")
            .locator
            .contains("unresolved")
    );
    let populations: BTreeSet<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.entity_id == id("lishu-commune"))
        .filter_map(|record| record.observation.population)
        .collect();
    assert_eq!(populations, BTreeSet::from([5590, 6124]));
    let caodi = input
        .data
        .records
        .iter()
        .find(|record| {
            record.entity_id == id("caodi-commune")
                && record.scan_page == Some(92)
                && record.source_date.is_some()
        })
        .unwrap();
    assert!(caodi.observation.seat_id.is_none());
    assert!(
        caodi
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("干家桥村")
    );
    assert!(
        table(&input, "shiba/zhongba")
            .observation
            .population
            .is_none()
    );
    assert!(
        table(&input, "shiba/zhongba")
            .observation
            .ethnicities
            .is_empty()
    );
}

#[test]
fn settlement_notes_and_river_reaches_keep_their_types_and_extent_evidence() {
    let input = input();
    for (key, kind) in [
        ("ganhegou/yangzhu-chang", EntityKind::Settlement),
        ("songlin/zhongchu-chang", EntityKind::Settlement),
        ("lishu/zhaobishan", EntityKind::Settlement),
        ("lishu/dashanying", EntityKind::Mountain),
        ("ganhegou/ganhe-power-station", EntityKind::Facility),
    ] {
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|entity| entity.id == id(key))
                .unwrap()
                .kind,
            kind
        );
    }
    for (key, extent) in [
        ("shiba/shiba-he", "上接南箐，下接草堤。境内流长45公里。"),
        ("ganhegou/gan-he", "源出鸭池乡，下入垮桥河。境内流长4公里。"),
        ("songlin/kuaqiao-he", "上接岔河，下接梨树，境内流长7公里。"),
        (
            "lishu/kuaqiao-he",
            "上接松林，下入飞雄流入大方。境内流长4公里。",
        ),
    ] {
        let observation = &table(&input, key).observation;
        assert_eq!(observation.extent.as_deref(), Some(extent));
        assert!(observation.coordinate.is_none());
        assert!(observation.population.is_none());
    }
    let names: BTreeSet<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.entity_id == id("songlin/kuaqiao-he"))
        .map(|record| record.observation.name.as_str())
        .collect();
    assert_eq!(names, BTreeSet::from(["垮桥河", "跨桥河"]));
    let bridge = input
        .data
        .records
        .iter()
        .find(|record| record.entity_id == id("shiba/weixian-qiao"))
        .unwrap();
    assert!(bridge.locator.contains("unresolved"));
    assert_ne!(
        bridge.entity_id,
        table(&input, "shiba/shiba-qiao").entity_id
    );
}

#[test]
fn ganhe_name_transition_preserves_full_year_uncertainty() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, mode, expected) in [
        (1957, TimeSelection::Possible, vec![]),
        (1959, TimeSelection::Certain, vec!["甘河公社"]),
        (1980, TimeSelection::Certain, vec!["甘河公社"]),
        (
            1981,
            TimeSelection::Possible,
            vec!["甘河公社", "甘河沟公社"],
        ),
        (1981, TimeSelection::Certain, vec![]),
        (1982, TimeSelection::Certain, vec![]),
        (1982, TimeSelection::Possible, vec!["甘河沟公社"]),
        (1982, TimeSelection::Nominal, vec!["甘河沟公社"]),
    ] {
        let slice = timeline.at(instant(year), mode).unwrap();
        let mut names: Vec<_> = slice
            .entities
            .iter()
            .filter(|entity| entity.entity_id == id("ganhegou-commune"))
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
}

#[test]
fn commune_origins_do_not_date_the_construction_records_as_completed_facilities() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, formed) in [(1957, false), (1959, true)] {
        let slice = timeline.at(instant(year), TimeSelection::Certain).unwrap();
        for name in ["shiba", "caodi", "ganhegou", "songlin", "lishu", "caoping"] {
            let count = slice
                .entities
                .iter()
                .find(|entity| entity.entity_id == id(&format!("{name}-commune")))
                .map(|entity| entity.origins.len())
                .unwrap_or_default();
            assert_eq!(count, usize::from(formed));
        }
    }
    let construction = input
        .data
        .records
        .iter()
        .find(|record| {
            record.entity_id == id("lishu/shuiniutun-power-station") && record.scan_page == Some(98)
        })
        .unwrap();
    assert_eq!(construction.source_date.as_ref().unwrap().text, "1975年");
    assert!(
        construction
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("动工兴建")
    );
    assert!(
        input
            .data
            .estimates
            .iter()
            .all(|estimate| estimate.entity_id != construction.entity_id)
    );
}
