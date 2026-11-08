use super::*;
use crate::input::load_blocking;
use atlas_core::calendar::{Calendar, CivilDate};
use atlas_ledger::{
    model::EntityKind,
    timeline::{TimeSelection, Timeline},
};

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
        .find(|r| r.key == format!("bijie-gazetteer/p{page}/{key}"))
        .unwrap()
}

#[cfg(test)]
fn names(input: &LedgerInput, key: &str, year: i64, selection: TimeSelection) -> BTreeSet<String> {
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
fn complete_tables_export_all_reviewed_rows_with_their_source_fields() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [
        (151, 34),
        (152, 3),
        (154, 13),
        (157, 20),
        (159, 37),
        (160, 10),
        (162, 36),
        (163, 13),
        (166, 34),
        (167, 6),
        (170, 28),
        (172, 28),
        (174, 25),
        (176, 12),
        (178, 13),
        (180, 19),
        (182, 35),
        (183, 6),
        (186, 14),
        (188, 18),
        (190, 14),
    ] {
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| r.source_id == "bijie-gazetteer" && r.scan_page == Some(page + 35))
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

#[path = "communes/tests.rs"]
mod communes;

#[path = "shaogang/tests.rs"]
mod shaogang;

#[path = "lanni/tests.rs"]
mod lanni;

#[path = "xiaoba/tests.rs"]
mod xiaoba;

#[test]
fn district_and_brigade_seats_follow_the_explicit_table_annotations() {
    let input = input();
    for (page, key, seat) in [
        (151, "haizijie-district", "haizijie/haizijie"),
        (151, "haizijie-commune", "haizijie/haizijie"),
        (151, "haizijie/haizijie-brigade", "haizijie/haizijie"),
        (154, "yingbin-commune", "yingbin/qingganglin"),
        (154, "yingbin/yingbin-brigade", "yingbin/fuji"),
        (154, "yingbin/changzheng-brigade", "yingbin/qingganglin"),
        (157, "guanyinqiao-commune", "guanyinqiao/qiaobian"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.seat_id,
            Some(id(seat))
        );
    }
    assert!(
        record(&input, 151, "haizijie-district")
            .observation
            .coordinate
            .is_none()
    );
    for (key, count) in [("haizijie", 9), ("yingbin", 2), ("guanyinqiao", 0)] {
        let brigades: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| {
                r.observation.parent_id == Some(id(&format!("{key}-commune")))
                    && r.observation.rank
                        == Some(atlas_core::assertion::AdministrativeRank::Brigade)
            })
            .collect();
        assert_eq!(brigades.len(), count);
        assert!(brigades.iter().all(|r| r.observation.seat_id.is_some()));
    }
    let settlements: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|r| {
            r.scan_page == Some(192)
                && r.observation.population.is_some()
                && r.observation.rank.is_none()
        })
        .collect();
    assert_eq!(settlements.len(), 14);
    assert!(
        settlements
            .iter()
            .all(|r| r.observation.parent_id == Some(id("guanyinqiao-commune")))
    );
}

#[test]
fn anomalous_coordinates_and_missing_populations_are_not_corrected() {
    let input = input();
    let coordinate = record(&input, 151, "haizijie/caojia-yuanzi")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(coordinate.latitude_text, "23°22′");
    assert!((coordinate.point.latitude - (23.0 + 22.0 / 60.0)).abs() < 1e-12);
    assert_eq!(coordinate.resolution_arcminutes, 1.0);
    let baijiafen = &record(&input, 154, "yingbin/baijiafen").observation;
    assert!(baijiafen.population.is_none());
    assert!(baijiafen.ethnicities.is_empty());
    assert_eq!(baijiafen.note.as_deref(), Some("村，人口并入真武山"));
    assert_eq!(
        record(&input, 154, "yingbin/zhenwushan")
            .observation
            .population,
        Some(217)
    );
    assert_eq!(
        record(&input, 153, "yingbin-commune/population")
            .observation
            .population,
        Some(3135)
    );
    assert_eq!(
        record(&input, 153, "yingbin-commune/population")
            .observation
            .households,
        Some(642)
    );
    assert_eq!(
        record(&input, 154, "yingbin-commune")
            .observation
            .population,
        Some(3079)
    );
    assert!(
        record(&input, 154, "yingbin-commune")
            .observation
            .households
            .is_none()
    );
}

#[test]
fn printed_spellings_and_bridge_notation_survive_publication() {
    let input = input();
    for (key, pinyin) in [
        ("yingbin-commune", "Yīnbīn Gōngshè"),
        ("yingbin/yingbin-brigade", "Yīnbīu Dàduì"),
        ("yingbin/maliupo", "Maliupō"),
        ("yingbin/zhenwushan", "Zhēnwushān"),
    ] {
        assert_eq!(
            record(&input, 154, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    assert_eq!(
        record(&input, 151, "haizijie/baiyanglin-brigade")
            .observation
            .name,
        "柏杨林大队"
    );
    assert_eq!(
        record(&input, 153, "yingbin/qingganglin/seat").entity_id,
        id("yingbin/qingganglin")
    );
    assert_eq!(
        record(&input, 153, "yingbin/qingganglin/seat")
            .observation
            .name,
        "青岗林"
    );
    assert_eq!(
        record(&input, 154, "yingbin/qingganglin").observation.name,
        "青杠林"
    );
    assert_eq!(
        record(&input, 151, "haizijie/qiansuo-bridge")
            .observation
            .note
            .as_deref(),
        Some("桥 (14—7)/20")
    );
    assert_eq!(
        record(&input, 157, "guanyinqiao/guanyin-bridge")
            .observation
            .note
            .as_deref(),
        Some("公路石桥 (11—7)/15")
    );
}

#[test]
fn river_identity_spans_communes_but_bridge_and_settlement_remain_distinct() {
    let input = input();
    for (page, key) in [
        (31, "dehou/guanyinqiao-river"),
        (156, "guanyinqiao-river"),
        (157, "guanyinqiao-river"),
    ] {
        let river = record(&input, page, key);
        assert_eq!(river.entity_id, id("guanyinqiao-river"));
        assert!(river.observation.coordinate.is_none());
    }
    let river = &record(&input, 156, "guanyinqiao-river").observation;
    assert_eq!(river.name, "田坝桥河");
    assert_eq!(river.former_names[0].name, "威镇河");
    let place = record(&input, 157, "guanyinqiao/guanyinqiao");
    let bridge = record(&input, 157, "guanyinqiao/guanyin-bridge");
    assert_eq!(place.observation.name, bridge.observation.name);
    assert_ne!(place.entity_id, bridge.entity_id);
    for (record, kind) in [
        (place, EntityKind::Settlement),
        (bridge, EntityKind::Bridge),
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
}

#[test]
fn names_follow_dated_changes_without_collapsing_the_renaming_period() {
    let input = input();
    for (key, year, expected) in [
        ("haizijie-district", 1951, "六区"),
        ("haizijie-district", 1953, "海子街区"),
        ("yingbin-commune", 1960, "福集公社"),
        ("yingbin-commune", 1982, "迎宾公社"),
        ("guanyinqiao-commune", 1960, "观音桥公社"),
        ("guanyinqiao-commune", 1978, "红卫公社"),
        ("guanyinqiao-commune", 1982, "观音桥公社"),
    ] {
        assert_eq!(
            names(&input, key, year, TimeSelection::Nominal),
            BTreeSet::from([expected.to_owned()])
        );
    }
    assert_eq!(
        names(&input, "guanyinqiao-commune", 1968, TimeSelection::Possible),
        BTreeSet::from(["观音桥公社".to_owned(), "红卫公社".to_owned()])
    );
    assert!(names(&input, "guanyinqiao-commune", 1968, TimeSelection::Certain).is_empty());
    assert_eq!(
        names(&input, "guanyinqiao-commune", 1978, TimeSelection::Certain),
        BTreeSet::from(["红卫公社".to_owned()])
    );
    let source = record(&input, 156, "guanyinqiao-commune/hongwei-renaming")
        .source_date
        .as_ref()
        .unwrap();
    assert_eq!(source.text, "文革期间");
    assert!(
        source
            .conversion_basis
            .as_ref()
            .unwrap()
            .contains("section 19")
    );
    assert!(
        input
            .catalog
            .sources
            .iter()
            .any(|s| s.id == "ccp-history-resolution-1981" && s.url.is_some())
    );
}
