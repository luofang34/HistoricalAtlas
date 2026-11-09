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

#[cfg(test)]
fn parents(input: &LedgerInput, key: &str, year: i64, mode: TimeSelection) -> BTreeSet<EntityId> {
    let slice = Timeline::new(&input.catalog, &input.data)
        .unwrap()
        .at(instant(year), mode)
        .unwrap();
    slice
        .entities
        .iter()
        .filter(|entity| entity.entity_id == id(key))
        .flat_map(|entity| &entity.properties)
        .flat_map(|property| &property.candidates)
        .filter_map(|candidate| match &candidate.assertion.value {
            TemporalValue::Hierarchy(value) => value.parent_id,
            _ => None,
        })
        .collect()
}

#[test]
fn all_commune_table_rows_round_trip_with_original_fields() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (82, 37),
        (83, 19),
        (87, 37),
        (90, 37),
        (91, 12),
        (94, 36),
        (95, 7),
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
fn district_contains_all_thirty_brigades_and_cross_page_seats() {
    let input = input();
    let mut total = 0_usize;
    for (commune, count) in [
        ("qingchang", 7),
        ("qingsong", 6),
        ("dahe", 5),
        ("kuaile", 3),
        ("yejiao", 4),
        ("baojia", 5),
    ] {
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
        total += brigades.len();
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
    assert_eq!(total, 30);
    assert_eq!(
        record(&input, 90, "yejiao/dashui-brigade")
            .observation
            .seat_id,
        Some(record(&input, 91, "yejiao/dashuigou").entity_id)
    );
    assert_eq!(
        record(&input, 94, "baojia/yudongchong-brigade")
            .observation
            .seat_id,
        Some(record(&input, 95, "baojia/yudongchong").entity_id)
    );
}

#[test]
fn coincident_bridges_and_settlement_groups_keep_separate_identities() {
    let input = input();
    let first = record(&input, 83, "dahe/dahe-bridge-30m");
    let second = record(&input, 83, "dahe/dahe-bridge-48m");
    assert_eq!(first.observation.name, second.observation.name);
    assert_eq!(first.observation.coordinate, second.observation.coordinate);
    assert_ne!(first.entity_id, second.entity_id);
    assert_eq!(
        first.observation.note.as_deref(),
        Some("公路桥 (30.4–5.5)/15")
    );
    assert_eq!(
        second.observation.note.as_deref(),
        Some("公路桥 (48.5–5)/15")
    );
    let village = record(&input, 94, "baojia/laowangchong");
    let area = record(&input, 94, "baojia/laowangchong-area");
    assert_eq!(village.observation.coordinate, area.observation.coordinate);
    assert_ne!(village.entity_id, area.entity_id);
    assert_eq!(village.observation.population, Some(126));
    assert_eq!(area.observation.population, None);
    assert_eq!(area.observation.note.as_deref(), Some("片村"));
    let places: BTreeSet<_> = input
        .data
        .records
        .iter()
        .filter(|record| {
            record.observation.name == "张家院子" && record.key.starts_with("bijie-gazetteer/p8")
        })
        .map(|record| record.entity_id)
        .collect();
    assert_eq!(
        places,
        BTreeSet::from([
            id("dahe/laluo-zhangjia-yuanzi"),
            id("dahe/keluoyuan-zhangjia-yuanzi")
        ])
    );
}

#[test]
fn blank_cells_printed_pinyin_and_unresolved_names_survive_export() {
    let input = input();
    for (page, key) in [
        (82, "dahe/guanfang-pingzi"),
        (87, "kuaile/xujia-matang"),
        (87, "kuaile/sheduoluo"),
    ] {
        assert!(record(&input, page, key).observation.ethnicities.is_empty());
    }
    let unnamed_type = input
        .data
        .entities
        .iter()
        .find(|entity| entity.id == id("yejiao/wangjiazhai"))
        .unwrap();
    assert_eq!(unnamed_type.kind, EntityKind::Toponym);
    assert!(
        record(&input, 91, "yejiao/wangjiazhai")
            .observation
            .population
            .is_none()
    );
    for (page, key, pinyin) in [
        (87, "kuaile-commune", "Kuàliè Gōngshè"),
        (94, "baojia/dadi", "Càdì"),
        (90, "yejiao/hujia-liangzi", "Fújiāliángzǐ"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    for (page, key) in [
        (87, "kuaile/jiamo"),
        (90, "yejiao/haijiaqing"),
        (90, "yejiao/shangjiadai"),
        (90, "yejiao/xiajiadai"),
    ] {
        let row = record(&input, page, key);
        assert_eq!(
            row.observation.pinyin.as_deref(),
            Some(row.observation.name.as_str())
        );
        assert!(row.locator.contains("unresolved glyph"));
    }
    let coordinate = record(&input, 87, "kuaile/daposhang")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(coordinate.longitude_text, "105°00,");
    assert_eq!(coordinate.point.longitude, 105.0);
    assert_eq!(coordinate.datum, atlas_geo::coordinates::Datum::Unspecified);
    assert_eq!(coordinate.resolution_arcminutes, 1.0);
}

#[test]
fn source_river_extents_and_named_landforms_retain_their_scopes() {
    let input = input();
    assert!(
        record(&input, 83, "dahe/dahe-river")
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约10公里")
    );
    let reach = record(&input, 95, "baojia/chahe-river");
    let extent = reach.observation.extent.as_ref().unwrap();
    assert!(extent.contains("约8公里"));
    assert!(extent.contains("全长7公里"));
    assert!(reach.observation.coordinate.is_none());
    let stream = record(&input, 89, "yejiao/dashuigou-stream");
    let village = record(&input, 91, "yejiao/dashuigou");
    assert_ne!(stream.entity_id, village.entity_id);
    assert!(stream.observation.coordinate.is_none());
    assert!(village.observation.coordinate.is_some());
    let mountain = input
        .data
        .entities
        .iter()
        .find(|entity| entity.id == id("yejiao/hujia-liangzi"))
        .unwrap();
    assert_eq!(mountain.kind, EntityKind::Mountain);
    assert!(
        record(&input, 89, "yejiao/hujia-liangzi")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("2109.4米")
    );
    assert!(
        record(&input, 90, "yejiao/hujia-liangzi")
            .observation
            .coordinate
            .is_some()
    );
}

#[test]
fn merger_interval_has_no_independent_commune_hierarchy() {
    let input = input();
    for (year, parent) in [
        (1960, Some("yangjiawan-district")),
        (1962, None),
        (1964, Some("yangjiawan-district")),
        (1967, Some("qingchang-district")),
    ] {
        let expected = parent.into_iter().map(id).collect();
        assert_eq!(
            parents(&input, "kuaile-commune", year, TimeSelection::Nominal),
            expected
        );
    }
    for (commune, parent) in [
        ("dahe", "yangjiawan-district"),
        ("yejiao", "changchunbao-district"),
        ("baojia", "heguantun-district"),
    ] {
        assert_eq!(
            parents(
                &input,
                &format!("{commune}-commune"),
                1960,
                TimeSelection::Nominal
            ),
            BTreeSet::from([id(parent)])
        );
        assert_eq!(
            parents(
                &input,
                &format!("{commune}-commune"),
                1967,
                TimeSelection::Nominal
            ),
            BTreeSet::from([id("qingchang-district")])
        );
    }
}

#[test]
fn conflicting_transfer_years_publish_both_claims_and_change_possible_selection() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    let key = id("kuaile-commune");
    let shard: serde_json::Value = serde_json::from_slice(
        &output[&PathBuf::from(format!("entities/index/{}.json", key.shard()))],
    )
    .unwrap();
    let disputed = &shard[key.0.to_string()]["disputes"][0];
    assert_eq!(disputed["alternatives"].as_array().unwrap().len(), 2);
    let dispute = input
        .data
        .disputes
        .iter()
        .find(|dispute| dispute.entity_id == key)
        .unwrap();
    let claims: BTreeSet<_> = input
        .data
        .records
        .iter()
        .filter(|record| dispute.alternatives.contains(&record.id))
        .map(|record| record.source_date.as_ref().unwrap().text.as_str())
        .collect();
    assert_eq!(claims, BTreeSet::from(["1965年", "1966年"]));
    assert_eq!(
        parents(&input, "kuaile-commune", 1965, TimeSelection::Possible),
        BTreeSet::from([id("yangjiawan-district"), id("qingchang-district")])
    );
    assert_eq!(
        parents(&input, "kuaile-commune", 1965, TimeSelection::Nominal),
        BTreeSet::from([id("yangjiawan-district")])
    );
}

#[test]
fn establishments_and_unfinished_reservoir_preserve_different_date_meanings() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, count) in [(1957, 0), (1959, 1)] {
        let slice = timeline.at(instant(year), TimeSelection::Certain).unwrap();
        for commune in ["dahe", "kuaile", "yejiao", "baojia"] {
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
    let reservoir = record(&input, 93, "baojia/xiangshui-reservoir");
    assert!(
        reservoir
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("已完成50%")
    );
    assert!(reservoir.source_date.is_none());
    assert!(
        input
            .data
            .estimates
            .iter()
            .all(|estimate| estimate.entity_id != reservoir.entity_id)
    );
    let extent = input
        .data
        .records
        .iter()
        .find(|record| record.key == "bijie-gazetteer/p93/baojia-commune/extent")
        .unwrap();
    assert!(
        extent
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("175°03′")
    );
    assert!(extent.observation.coordinate.is_none());
}
