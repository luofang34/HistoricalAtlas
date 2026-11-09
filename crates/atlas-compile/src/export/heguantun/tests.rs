use super::*;
use crate::input::load_blocking;
use atlas_core::calendar::{Calendar, CivilDate};
use atlas_ledger::timeline::{TimeSelection, Timeline};

#[path = "communes/tests.rs"]
mod communes;

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
        .find(|row| {
            row.entity_id == id(key)
                && row.source_id == "bijie-gazetteer"
                && row.scan_page == Some(page + 35)
        })
        .unwrap()
}

#[cfg(test)]
fn names(input: &LedgerInput, key: &str, year: i64, month: u8) -> BTreeSet<String> {
    let at = CivilDate {
        year,
        month,
        day: 15,
    }
    .to_epoch(Calendar::Gregorian, 0)
    .unwrap();
    Timeline::new(&input.catalog, &input.data)
        .unwrap()
        .at(at, TimeSelection::Nominal)
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
fn complete_tables_publish_each_reviewed_row_without_changing_its_fields() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (127, 37),
        (128, 15),
        (130, 37),
        (131, 37),
        (132, 11),
        (134, 37),
        (135, 35),
        (136, 2),
        (138, 32),
        (140, 37),
        (141, 6),
        (143, 35),
        (144, 36),
        (145, 9),
    ] {
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|row| row.source_id == "bijie-gazetteer" && row.scan_page == Some(page + 35))
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
fn commune_hierarchies_keep_continued_rows_and_only_explicit_seats() {
    let input = input();
    for (commune, count, seats) in [
        ("heguantun", 7, 7),
        ("duipo", 8, 8),
        ("shuiqing", 9, 8),
        ("daba", 4, 4),
        ("shanzhai", 4, 4),
        ("houqing", 11, 11),
    ] {
        let brigades: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|row| {
                row.observation.parent_id == Some(id(&format!("{commune}-commune")))
                    && row.observation.rank
                        == Some(atlas_core::assertion::AdministrativeRank::Brigade)
            })
            .collect();
        assert_eq!(brigades.len(), count);
        assert_eq!(
            brigades
                .iter()
                .filter(|row| row.observation.seat_id.is_some())
                .count(),
            seats
        );
        for brigade in brigades {
            if let Some(seat) = brigade.observation.seat_id {
                assert!(input.data.records.iter().any(|row| row.entity_id == seat
                    && row.observation.parent_id == Some(brigade.entity_id)));
            }
        }
    }
    assert_eq!(
        record(&input, 131, "duipo/zhongxin-brigade")
            .observation
            .seat_id,
        Some(record(&input, 132, "duipo/dazhai").entity_id)
    );
    assert_eq!(
        record(&input, 128, "heguantun/xiazhai")
            .observation
            .parent_id,
        Some(id("heguantun/maomaodong-brigade"))
    );
    assert_eq!(
        record(&input, 135, "shuiqing/dianzishang")
            .observation
            .parent_id,
        Some(id("shuiqing/maanqiao-brigade"))
    );
    assert!(
        record(&input, 135, "shuiqing/shangdawan-brigade")
            .observation
            .seat_id
            .is_none()
    );
    let district = record(&input, 127, "heguantun-district");
    assert!(district.observation.coordinate.is_none());
    assert!(district.observation.population.is_none());
    assert_eq!(
        district.observation.seat_id,
        Some(id("heguantun/heguantun"))
    );
}

#[test]
fn outlying_and_unresolved_coordinates_keep_the_printed_notation() {
    let input = input();
    let brigade = record(&input, 127, "heguantun/heguantun-brigade")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    let seat = record(&input, 127, "heguantun/heguantun")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(brigade.latitude_text, "21°22′");
    assert_eq!(brigade.point.latitude, 21.0 + 22.0 / 60.0);
    assert_eq!(seat.latitude_text, "27°22′");
    for (page, key, text) in [
        (127, "heguantun-commune", "105.15′"),
        (130, "duipo/jindong", "105.18′"),
        (131, "duipo/zhaojiawanzi", "105.18′"),
    ] {
        let value = record(&input, page, key)
            .observation
            .coordinate
            .as_ref()
            .unwrap();
        assert_eq!(value.longitude_text, text);
        assert_eq!(value.resolution_arcminutes, 1.0);
    }
    let value = record(&input, 131, "duipo/longdong")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(value.latitude_text, "27.30′");
    assert_eq!(value.point.latitude, 27.5);
    let unresolved = &record(&input, 135, "shuiqing/xialiushuiyan").observation;
    assert!(unresolved.coordinate.is_none());
    assert_eq!(unresolved.population, Some(176));
    assert_eq!(
        unresolved
            .unresolved_coordinate
            .as_ref()
            .unwrap()
            .longitude_text,
        "105°87′"
    );
}

#[test]
fn homonymous_places_rocks_mountains_and_rivers_keep_independent_identities() {
    let input = input();
    for (page_a, key_a, page_b, key_b) in [
        (128, "heguantun/shisun", 128, "heguantun/shisun-rock"),
        (
            127,
            "heguantun/niezhaishan",
            128,
            "heguantun/niezhaishan-mountain",
        ),
        (131, "duipo/dalutang", 131, "duipo/dalutang-place"),
        (
            134,
            "shuiqing/shuiqingkou",
            134,
            "shuiqing/shuiqingkou-place",
        ),
        (
            134,
            "shuiqing/liaoyuan-luoshuidong",
            135,
            "shuiqing/maanqiao-luoshuidong",
        ),
        (135, "shuiqing/daganhe-place", 136, "shuiqing/dagan-river"),
    ] {
        let a = record(&input, page_a, key_a);
        let b = record(&input, page_b, key_b);
        assert_eq!(a.observation.name, b.observation.name);
        assert_ne!(a.entity_id, b.entity_id);
    }
    let rock = id("heguantun/shisun-rock");
    assert_eq!(
        input
            .data
            .entities
            .iter()
            .find(|entity| entity.id == rock)
            .unwrap()
            .kind,
        atlas_ledger::model::EntityKind::GeologicalSite
    );
    for (page, key, text) in [
        (128, "heguantun/guantun-river", "田坝桥"),
        (132, "duipo/mianyu-river", "约19公里"),
        (136, "shuiqing/dagan-river", "约5公里"),
    ] {
        let river = record(&input, page, key);
        assert!(river.observation.coordinate.is_none());
        assert!(river.observation.extent.as_ref().unwrap().contains(text));
    }
}

#[test]
fn source_spelling_blank_cells_and_unresolved_glyphs_survive_compilation() {
    let input = input();
    for (page, key, name, pinyin) in [
        (
            127,
            "heguantun/anazhai-brigade",
            "啊哪寨大队",
            "Ānāzhài Dàduì",
        ),
        (127, "heguantun/anazhai", "阿哪寨", "Ānāzhài"),
        (127, "heguantun/dahaizi", "大海子", "Dàbǎizǐ"),
        (127, "heguantun/yuejin-farm", "跃进农场", "Yuèjìn Lóngchǎng"),
        (131, "duipo/jiamu", "夏木", "Jiámù"),
        (134, "shuiqing/xinzhan", "新站", "Xīnshàn"),
    ] {
        let obs = &record(&input, page, key).observation;
        assert_eq!(obs.name, name);
        assert_eq!(obs.pinyin.as_deref(), Some(pinyin));
    }
    let glyph = record(&input, 135, "shuiqing/qingshikala");
    assert_eq!(glyph.observation.name, "Qīngshīkālā");
    assert!(glyph.locator.contains("unresolved glyph"));
    assert!(glyph.observation.note.as_ref().unwrap().contains("青石㮟"));
    assert!(
        record(&input, 127, "heguantun/yuejin-farm")
            .observation
            .population
            .is_none()
    );
    assert!(
        record(&input, 130, "duipo/guodimatang")
            .observation
            .ethnicities
            .is_empty()
    );
    assert_eq!(
        record(&input, 132, "duipo/dazhai").observation.ethnicities,
        ["南京"]
    );
}

#[test]
fn district_merger_does_not_backdate_the_commune_and_renaming_keeps_year_bounds() {
    let input = input();
    for (key, year, month, expected) in [
        ("duipo/former-district", 1952, 7, "十四区"),
        ("duipo/former-district", 1955, 7, "对坡区"),
        ("duipo/former-district", 1956, 2, "对坡区"),
        ("duipo-commune", 1959, 7, "对坡公社"),
        ("heguantun-commune", 1980, 7, "官屯公社"),
        ("heguantun-commune", 1982, 7, "何官屯公社"),
        ("heguantun-district", 1980, 7, "官屯区"),
        ("heguantun-district", 1982, 7, "何官屯区"),
        ("shuiqing-commune", 1959, 7, "水箐公社"),
    ] {
        assert_eq!(
            names(&input, key, year, month),
            BTreeSet::from([expected.into()])
        );
    }
    assert!(names(&input, "duipo/former-district", 1956, 4).is_empty());
    assert!(names(&input, "duipo-commune", 1951, 7).is_empty());
    assert!(names(&input, "duipo-commune", 1957, 7).is_empty());
    assert_ne!(id("duipo-commune"), id("duipo/former-district"));
    let merger = input
        .data
        .estimates
        .iter()
        .find(|value| value.key == "editorial/bijie/duipo/duipo-district-name")
        .unwrap();
    let EstimateValue::Temporal { assertion } = &merger.value else {
        panic!("Expected a name assertion")
    };
    assert_eq!(
        assertion.validity.end.unwrap().precision,
        atlas_core::time::TimePrecision::Month
    );
    let transfer = input
        .data
        .records
        .iter()
        .find(|row| row.key == "bijie-gazetteer/p133/shuiqing-commune/seat-relocation")
        .unwrap();
    assert!(transfer.source_date.is_none());
    assert_eq!(
        transfer.observation.seat_id,
        Some(id("shuiqing/chenjiayanjiao"))
    );
}
