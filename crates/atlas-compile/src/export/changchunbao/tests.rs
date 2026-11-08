use super::*;
use crate::input::load_blocking;
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
fn record<'a>(input: &'a LedgerInput, page: u32, key: &str) -> &'a Record {
    input
        .data
        .records
        .iter()
        .find(|record| {
            record.entity_id == id(key)
                && record.source_id == "bijie-gazetteer"
                && record.scan_page == Some(page + 35)
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

#[cfg(test)]
fn names(input: &LedgerInput, key: &str, year: i64, mode: TimeSelection) -> BTreeSet<String> {
    Timeline::new(&input.catalog, &input.data)
        .unwrap()
        .at(instant(year), mode)
        .unwrap()
        .entities
        .iter()
        .filter(|entity| entity.entity_id == id(key))
        .flat_map(|entity| &entity.properties)
        .flat_map(|property| &property.candidates)
        .filter_map(|candidate| match &candidate.assertion.value {
            TemporalValue::Name(value) => Some(value.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn table_pages_publish_all_rows_with_their_original_fields() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [
        (101, 37),
        (102, 29),
        (104, 36),
        (105, 9),
        (108, 33),
        (109, 1),
    ] {
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|record| {
                record.source_id == "bijie-gazetteer" && record.scan_page == Some(page + 35)
            })
            .collect();
        assert_eq!(rows.len(), count);
        for row in rows {
            assert!(row.checked);
            let path = PathBuf::from(format!("entities/index/{}.json", row.entity_id.shard()));
            let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
            let published = shard[row.entity_id.0.to_string()]["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|value| value["id"] == row.id.0.to_string())
                .unwrap();
            assert_eq!(published, &serde_json::to_value(row).unwrap());
        }
    }
}

#[test]
fn three_commune_hierarchies_keep_all_brigades_and_continued_seats() {
    let input = input();
    for (commune, count) in [("changchunbao", 9), ("gezuo", 8), ("qingfeng", 6)] {
        let brigades: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|record| {
                record.observation.parent_id == Some(id(&format!("{commune}-commune")))
                    && record.observation.rank
                        == Some(atlas_core::assertion::AdministrativeRank::Brigade)
            })
            .collect();
        assert_eq!(brigades.len(), count);
        for brigade in brigades {
            let seat = brigade.observation.seat_id.unwrap();
            assert!(input.data.records.iter().any(|record| {
                record.entity_id == seat && record.observation.parent_id == Some(brigade.entity_id)
            }));
        }
    }
    assert_eq!(
        record(&input, 101, "changchunbao/huashi-brigade")
            .observation
            .seat_id,
        Some(record(&input, 102, "changchunbao/huashiban").entity_id)
    );
    let district = record(&input, 101, "changchunbao-district");
    assert!(district.observation.coordinate.is_none());
    assert!(district.observation.population.is_none());
    assert_eq!(
        district.observation.seat_id,
        Some(id("changchunbao/changchunbao"))
    );
}

#[test]
fn coincident_villages_landforms_and_factories_keep_distinct_identities() {
    let input = input();
    for group in [
        vec![
            (101, "changchunbao/shigangling-coal", EntityKind::Factory),
            (101, "changchunbao/shigangling", EntityKind::Settlement),
            (101, "changchunbao/shigangling-place", EntityKind::Toponym),
        ],
        vec![
            (102, "changchunbao/haozidong-place", EntityKind::Toponym),
            (102, "changchunbao/haozidong-cave", EntityKind::Cave),
        ],
        vec![
            (104, "gezuo/shiyakou", EntityKind::Settlement),
            (105, "gezuo/shiyakou-mountain", EntityKind::Mountain),
        ],
        vec![
            (108, "qingfeng/daodongpo", EntityKind::Settlement),
            (108, "qingfeng/daodongpo-cave", EntityKind::Cave),
        ],
    ] {
        let mut identities = BTreeSet::new();
        let first = record(&input, group[0].0, group[0].1);
        for (page, key, kind) in &group {
            let row = record(&input, *page, key);
            assert!(identities.insert(row.entity_id));
            assert_eq!(row.observation.coordinate, first.observation.coordinate);
            assert_eq!(
                input
                    .data
                    .entities
                    .iter()
                    .find(|entity| entity.id == row.entity_id)
                    .unwrap()
                    .kind,
                *kind
            );
        }
    }
    let village = record(&input, 104, "gezuo/maanshan");
    let mountain = record(&input, 105, "gezuo/maanshan-mountain");
    assert_eq!(village.observation.name, mountain.observation.name);
    assert_ne!(village.entity_id, mountain.entity_id);
    assert_ne!(
        village.observation.coordinate,
        mountain.observation.coordinate
    );
}

#[test]
fn blank_cells_and_printed_spellings_are_not_normalized() {
    let input = input();
    for (page, key) in [
        (104, "gezuo/mujiazhai"),
        (108, "qingfeng/shamuba"),
        (108, "qingfeng/daodongpo"),
    ] {
        assert!(record(&input, page, key).observation.ethnicities.is_empty());
    }
    for (key, ethnicities) in [
        ("gezuo/yuantang-brigade", vec!["汉", "菜"]),
        ("gezuo/laoyazhai", vec!["汉", "菜"]),
        ("gezuo/zhongjiayuanzi", vec!["汉", "菜"]),
        ("gezuo/gezuo", vec!["汉", "南京"]),
        ("gezuo/xiongjia-brigade", vec!["汉", "京"]),
    ] {
        assert_eq!(
            record(&input, 104, key).observation.ethnicities,
            ethnicities
        );
    }
    for (page, key, pinyin) in [
        (101, "changchunbao/xujia", "Xú(方音读Xiòng)jiá"),
        (101, "changchunbao/gujiaying", "Gùjiāyín"),
        (102, "changchunbao/xiaodonggou", "Xiǎdònggōu"),
        (104, "gezuo/xiaozhai", "Xiàozhài"),
        (108, "qingfeng/dawa", "Dàwā"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    assert_eq!(
        record(&input, 108, "qingfeng/dawa").observation.name,
        "大垇"
    );
    assert_eq!(
        record(&input, 108, "qingfeng/dawazi").observation.name,
        "大垇子"
    );
    let area = record(&input, 104, "gezuo/xiaotun-area");
    assert!(area.observation.population.is_none());
    assert_eq!(area.observation.note.as_deref(), Some("片村"));
    let pond = record(&input, 105, "gezuo/xiongjia-pond");
    assert!(pond.observation.population.is_none());
    assert_eq!(pond.observation.extent.as_deref(), Some("面积约5亩"));
}

#[test]
fn overview_and_table_disagreements_remain_separate_observations() {
    let input = input();
    let population = input
        .data
        .records
        .iter()
        .find(|record| record.key == "bijie-gazetteer/p99/changchunbao-commune/population")
        .unwrap();
    assert_eq!(population.observation.households, Some(32269));
    assert_eq!(population.observation.population, Some(16721));
    assert_eq!(
        record(&input, 101, "changchunbao-commune")
            .observation
            .population,
        Some(15171)
    );
    for (page, length) in [(99, "约10公里"), (102, "约长9.5公里")] {
        let river = record(&input, page, "changchunbao/changchun-river");
        assert!(river.observation.extent.as_ref().unwrap().contains(length));
        assert!(river.observation.coordinate.is_none());
    }
    let summit = record(&input, 105, "gezuo/zengjiada-shan");
    assert_eq!(
        summit.observation.note.as_deref(),
        Some("山峰、海拔18351米")
    );
    assert_eq!(
        summit
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .latitude_text,
        "27°18′"
    );
    let construction = record(&input, 106, "qingfeng/songguan-reservoir");
    assert!(
        construction
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("约200万方")
    );
    assert_eq!(construction.source_date.as_ref().unwrap().text, "1958年");
    assert!(
        record(&input, 109, "qingfeng/songguan-reservoir")
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("205万立方米")
    );
    assert!(
        input
            .data
            .estimates
            .iter()
            .all(|estimate| estimate.entity_id != construction.entity_id)
    );
}

#[test]
fn dated_names_change_at_the_cited_years_with_uncertainty() {
    let input = input();
    for (key, year, before, after) in [
        ("changchunbao-district", 1981, "长春区", "长春堡区"),
        ("changchunbao-commune", 1981, "长春公社", "长春堡公社"),
        ("qingfeng-commune", 1966, "丰乐公社", "清丰公社"),
    ] {
        assert_eq!(
            names(&input, key, year - 1, TimeSelection::Nominal),
            BTreeSet::from([before.into()])
        );
        assert_eq!(
            names(&input, key, year + 1, TimeSelection::Nominal),
            BTreeSet::from([after.into()])
        );
        assert_eq!(
            names(&input, key, year, TimeSelection::Possible),
            BTreeSet::from([before.into(), after.into()])
        );
        assert!(names(&input, key, year, TimeSelection::Certain).is_empty());
        assert!(names(&input, key, year + 1, TimeSelection::Certain).is_empty());
        let certain_before = names(&input, key, year - 1, TimeSelection::Certain);
        if key == "changchunbao-district" {
            assert!(certain_before.is_empty());
        } else {
            assert_eq!(certain_before, BTreeSet::from([before.into()]));
        }
    }
    assert!(names(&input, "qingfeng-commune", 1957, TimeSelection::Possible).is_empty());
    assert!(
        names(
            &input,
            "changchunbao-commune",
            1957,
            TimeSelection::Possible
        )
        .is_empty()
    );
}

#[test]
fn establishment_dates_and_absorbed_communes_have_separate_meanings() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, count) in [(1957, 0), (1959, 1)] {
        let slice = timeline.at(instant(year), TimeSelection::Certain).unwrap();
        for commune in ["changchunbao", "gezuo", "qingfeng"] {
            let entity = id(&format!("{commune}-commune"));
            let origins = slice
                .entities
                .iter()
                .find(|item| item.entity_id == entity)
                .map(|item| item.origins.len())
                .unwrap_or_default();
            assert_eq!(origins, count);
        }
    }
    for (page, commune, brigade) in [
        (
            103,
            "gezuo/xiaolong-former-commune",
            "gezuo/xiaolong-brigade",
        ),
        (103, "gezuo/yangliu-former-commune", "gezuo/yangliu-brigade"),
        (
            106,
            "qingfeng/qingtang-former-commune",
            "changchunbao/qingtang-brigade",
        ),
    ] {
        let row = record(&input, page, commune);
        assert_ne!(row.entity_id, id(brigade));
        assert_eq!(row.source_date.as_ref().unwrap().text, "1966年");
        assert!(row.observation.parent_id.is_none());
        assert!(
            input
                .data
                .estimates
                .iter()
                .all(|estimate| estimate.entity_id != row.entity_id)
        );
    }
}
