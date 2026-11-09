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

#[test]
fn reviewed_tables_and_continued_elevation_round_trip_through_publication() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [(74, 36), (75, 36), (76, 8), (78, 36), (79, 35), (80, 2)] {
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
    let elevation = record(&input, 76, "qingchang/yingzui-yan");
    assert!(elevation.locator.contains("continuation"));
    assert_eq!(elevation.observation.note.as_deref(), Some("1715.7米。"));
    assert!(elevation.observation.coordinate.is_none());
    assert!(
        record(&input, 75, "qingchang/yingzui-yan")
            .observation
            .coordinate
            .is_some()
    );
}

#[test]
fn source_hierarchy_preserves_every_brigade_and_cross_page_seat() {
    let input = input();
    for (commune, count) in [("qingchang", 7), ("qingsong", 6)] {
        let parent = id(&format!("{commune}-commune"));
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
        for brigade in brigades {
            let seat = brigade.observation.seat_id.unwrap();
            assert!(
                input
                    .data
                    .records
                    .iter()
                    .any(|record| record.entity_id == seat
                        && record.observation.parent_id == Some(brigade.entity_id))
            );
        }
    }
    assert_eq!(
        record(&input, 78, "qingsong-commune").observation.seat_id,
        Some(id("qingsong/shangzhai"))
    );
    assert_eq!(
        record(&input, 78, "qingsong/qinggangling-brigade")
            .observation
            .seat_id,
        Some(id("qingsong/qinggangling"))
    );
    assert_eq!(
        record(&input, 75, "qingchang/fanjia-huoshan")
            .observation
            .parent_id,
        Some(id("qingchang/xingou-brigade"))
    );
    assert_eq!(
        record(&input, 79, "qingsong/chuduping")
            .observation
            .parent_id,
        Some(id("qingsong/qinggangling-brigade"))
    );
}

#[test]
fn printed_population_coordinates_and_blank_cells_remain_independent() {
    let input = input();
    assert_eq!(
        record(&input, 74, "qingchang-commune")
            .observation
            .population,
        Some(12778)
    );
    assert_eq!(
        record(&input, 72, "qingchang-commune")
            .observation
            .population,
        Some(13426)
    );
    let brigade = &record(&input, 79, "qingsong/chuduhe-brigade").observation;
    let coordinate = brigade.coordinate.as_ref().unwrap();
    assert_eq!(coordinate.latitude_text, "27°2′");
    assert!((coordinate.point.latitude - (27.0 + 2.0 / 60.0)).abs() < 1e-12);
    assert_eq!(coordinate.resolution_arcminutes, 1.0);
    assert_eq!(coordinate.datum, atlas_geo::coordinates::Datum::Unspecified);
    assert!(
        record(&input, 78, "qingsong/leijia-matang")
            .observation
            .ethnicities
            .is_empty()
    );
    assert!(
        record(&input, 78, "qingsong/xiao-yakou")
            .observation
            .population
            .is_none()
    );
    assert_eq!(
        record(&input, 74, "qingchang/wujiatun")
            .observation
            .ethnicities,
        vec!["汉", "南京"]
    );
    for (page, key, name) in [
        (78, "qingsong/kalang", "Kálang"),
        (79, "qingsong/kalanggou", "Kálanggōu"),
    ] {
        let row = record(&input, page, key);
        assert_eq!(row.observation.name, name);
        assert!(row.locator.contains("unresolved glyphs"));
    }
}

#[test]
fn same_name_localities_retain_types_and_distinct_identities() {
    let input = input();
    for (key, kind) in [
        ("qingchang/chenjia-gou", EntityKind::Settlement),
        ("qingchang/chenjia-gou-toponym", EntityKind::Toponym),
        ("qingsong/miaoying-dashan-village", EntityKind::Settlement),
        ("qingsong/miaoying-dashan", EntityKind::Mountain),
        ("qingchang/shanyulan", EntityKind::Tree),
        ("qingchang/neolithic-site", EntityKind::HistoricSite),
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
    let village = record(&input, 79, "qingsong/miaoying-dashan-village");
    let mountain = record(&input, 79, "qingsong/miaoying-dashan");
    assert_eq!(village.observation.name, mountain.observation.name);
    assert_eq!(
        village.observation.coordinate,
        mountain.observation.coordinate
    );
    assert_ne!(village.entity_id, mountain.entity_id);
    assert_eq!(village.observation.population, Some(115));
    assert!(mountain.observation.population.is_none());
    assert!(
        mountain
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("2137.6")
    );
}

#[test]
fn river_lengths_preserve_their_extent_and_name_scopes() {
    let input = input();
    let reach = record(&input, 76, "qingchang/qingchang-he");
    let river = record(&input, 71, "qingchang-district/qingchang-he");
    assert_ne!(reach.entity_id, river.entity_id);
    assert!(
        reach
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("10公里")
    );
    assert!(
        river
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("22公里")
    );
    let names: BTreeSet<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.entity_id == id("qingsong/dahe"))
        .map(|record| record.observation.name.as_str())
        .collect();
    assert_eq!(names, BTreeSet::from(["大河", "青松河"]));
    assert!(
        record(&input, 80, "qingsong/dahe")
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约8公里")
    );
}

#[test]
fn six_communes_join_the_district_with_year_level_uncertainty() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, selection, count) in [
        (1964, TimeSelection::Possible, 0),
        (1965, TimeSelection::Possible, 1),
        (1966, TimeSelection::Possible, 6),
        (1967, TimeSelection::Nominal, 6),
    ] {
        let slice = timeline.at(instant(year), selection).unwrap();
        let children: BTreeSet<_>=slice.entities.iter().filter(|entity| entity.properties.iter()
            .flat_map(|property| &property.candidates).any(|candidate| matches!(&candidate.assertion.value,
                TemporalValue::Hierarchy(value) if value.parent_id == Some(id("qingchang-district")))))
            .map(|entity| entity.entity_id).collect();
        assert_eq!(children.len(), count);
        if count == 6 {
            assert_eq!(
                children,
                [
                    "qingchang",
                    "qingsong",
                    "dahe",
                    "kuaile",
                    "yejiao",
                    "baojia"
                ]
                .into_iter()
                .map(|name| id(&format!("{name}-commune")))
                .collect()
            );
        }
    }
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|estimate| estimate.key == "editorial/bijie/qingsong/qingchang-district-membership")
        .unwrap();
    let EstimateValue::Temporal { assertion } = &estimate.value else {
        panic!("expected hierarchy")
    };
    assert_eq!(
        assertion.validity.start.as_ref().unwrap().precision,
        atlas_core::time::TimePrecision::Year
    );
    assert!(assertion.validity.end.is_none());
}

#[test]
fn establishment_dates_do_not_turn_tree_protection_or_construction_into_origins() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, communes, district) in [
        (1957, false, false),
        (1959, true, false),
        (1967, true, true),
    ] {
        let slice = timeline.at(instant(year), TimeSelection::Certain).unwrap();
        for (key, formed) in [
            ("qingchang-commune", communes),
            ("qingsong-commune", communes),
            ("qingchang-district", district),
        ] {
            let count = slice
                .entities
                .iter()
                .find(|entity| entity.entity_id == id(key))
                .map(|entity| entity.origins.len())
                .unwrap_or_default();
            assert_eq!(count, usize::from(formed));
        }
    }
    for key in ["qingchang/shanyulan", "qingsong/power-station"] {
        assert!(
            input
                .data
                .estimates
                .iter()
                .all(|estimate| estimate.entity_id != id(key))
        );
    }
    let construction = record(&input, 77, "qingsong/power-station");
    assert_eq!(construction.source_date.as_ref().unwrap().text, "1978年");
    assert!(
        construction
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("完成50%以上")
    );
}
