use super::*;
use atlas_core::{assertion::TemporalValue, origin::OriginTime};

#[test]
fn complete_pages_publish_and_retain_the_two_commune_anchor_identities() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (289, 13),
        (290, 33),
        (291, 2),
        (293, 18),
        (294, 1),
        (295, 37),
        (296, 14),
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
            assert_eq!(
                crate::export::tests::published_record(&output, row),
                serde_json::to_value(row).unwrap()
            );
        }
    }
    for (key, expected) in [
        (
            "page-290/yongxing-commune",
            "8466de41-1761-586f-b497-c693e0b7aabc",
        ),
        (
            "page-295/cengtai-commune",
            "8e8bee70-0380-52a6-8e3c-5090cd663d71",
        ),
    ] {
        assert_eq!(record(&input, key).id.0.to_string(), expected);
    }
}

#[test]
fn networks_preserve_the_reviewed_residents_brigades_and_seats() {
    let input = input();
    for (commune, nodes, settlements, brigades, seats) in
        [("yongxing", 38, 24, 5, 6), ("cengtai", 54, 38, 7, 8)]
    {
        let network = NodalBuilder::new(&input.catalog, &input.data)
            .unwrap()
            .build(NodalQuery {
                entity_id: id(&format!("{commune}-commune")),
                scope: NodalScope::Source {
                    source_id: "bijie-gazetteer".into(),
                },
            })
            .unwrap();
        assert_eq!(network.nodes.len(), nodes);
        assert_eq!(
            network
                .nodes
                .iter()
                .filter(|n| n.anchors.is_empty())
                .count(),
            4
        );
        let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
        for (kind, count) in [
            (EntityKind::Settlement, settlements),
            (EntityKind::Brigade, brigades),
        ] {
            assert_eq!(
                input
                    .data
                    .entities
                    .iter()
                    .filter(|e| members.contains(&e.id) && e.kind == kind)
                    .count(),
                count
            );
        }
        assert_eq!(
            network
                .links
                .iter()
                .filter(|l| l.relation == RelationKind::Seat)
                .map(|l| (l.from, l.to))
                .collect::<BTreeSet<_>>()
                .len(),
            seats
        );
    }
    for key in ["tangfang", "yangjiaguan", "meizigou"] {
        assert_eq!(
            record(&input, &format!("p296/cengtai/qingchun/{key}"))
                .observation
                .parent_id,
            Some(id("cengtai/qingchun-brigade"))
        );
    }
    assert_eq!(
        record(&input, "page-290/yongxing-commune")
            .observation
            .seat_id,
        Some(id("yongxing/yongxing/yanjiao"))
    );
    assert_eq!(
        record(&input, "page-295/cengtai-commune")
            .observation
            .seat_id,
        Some(id("cengtai/cengtai/cengtaijie"))
    );
}

#[test]
fn coincident_paper_mill_gullies_and_same_named_villages_keep_separate_identities() {
    let input = input();
    let seat = record(&input, "p295/cengtai/guanghui/zhifanggou");
    let village = record(&input, "p295/cengtai/guanghui/zhifanggou-village");
    assert_eq!(seat.observation.name, village.observation.name);
    assert_ne!(seat.entity_id, village.entity_id);
    assert_eq!(seat.observation.coordinate, village.observation.coordinate);
    assert_eq!(seat.observation.parent_id, village.observation.parent_id);
    assert_eq!(seat.observation.population, Some(567));
    assert!(village.observation.population.is_none());
    assert!(village.observation.ethnicities.is_empty());
    assert_eq!(village.observation.note.as_deref(), Some("村"));
    assert_eq!(
        record(&input, "p295/cengtai/guanghui-brigade")
            .observation
            .seat_id,
        Some(seat.entity_id)
    );
    for (a, b) in [
        (
            "p295/cengtai/donggou/meizigou",
            "p296/cengtai/qingchun/meizigou",
        ),
        (
            "p295/cengtai/xiangyang/beishan",
            "p296/cengtai/guanxiang/beishan",
        ),
    ] {
        let (a, b) = (record(&input, a), record(&input, b));
        assert_eq!(a.observation.name, b.observation.name);
        assert_ne!(a.entity_id, b.entity_id);
        assert_ne!(a.observation.parent_id, b.observation.parent_id);
        assert_ne!(a.observation.coordinate, b.observation.coordinate);
    }
    for (key, kind, population, note) in [
        (
            "p296/cengtai/qingchun/tangfang",
            EntityKind::Toponym,
            Some(74),
            "地",
        ),
        (
            "p295/cengtai/donggou/cengtai-school",
            EntityKind::Settlement,
            None,
            "村",
        ),
        (
            "p296/cengtai/guanxiang/xieshanqing",
            EntityKind::Toponym,
            None,
            "地名",
        ),
    ] {
        let row = record(&input, key);
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|e| e.id == row.entity_id)
                .unwrap()
                .kind,
            kind
        );
        assert_eq!(row.observation.population, population);
        assert_eq!(row.observation.note.as_deref(), Some(note));
    }
}

#[test]
fn printed_pinyin_missing_demographics_and_outlying_minutes_survive_export() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (key, expected) in [
        ("p290/yongxing/yongxing/lijiazhai", "Jìjiāzhài"),
        ("p290/yongxing/yongfeng/gaoqiao", "Gāogqiào"),
        ("p290/yongxing/yongping/huoshaobao", "Huǒshāopù"),
        ("p295/cengtai/donggou/cengtai-school", "GéngtàiXiǒoxué"),
        ("p295/cengtai/hongyan/shangdumuqiao", "Shàngdmùqiào"),
        ("p295/cengtai/hongyan/xiadumuqiao", "Xiàodùmùqiào"),
        ("p296/cengtai/guanxiang/guanxiangcheng", "Guānxiāngchéug"),
        ("p296/cengtai/guanxiang/xieshanqing", "Xièshāngǐng"),
    ] {
        assert_eq!(
            crate::export::tests::published_record(&output, record(&input, key))["observation"]["pinyin"],
            expected
        );
    }
    for (key, longitude, latitude) in [
        ("p290/yongxing/yuqing/zhanhuodi", "105°28′", "27°28′"),
        ("p295/cengtai/hongyan/shangdumuqiao", "105°24′", "27°28′"),
        ("p296/cengtai/guanxiang/xieshanqing", "105°24′", "27°29′"),
    ] {
        let coordinate = record(&input, key).observation.coordinate.as_ref().unwrap();
        assert_eq!(coordinate.longitude_text, longitude);
        assert_eq!(coordinate.latitude_text, latitude);
        assert_eq!(coordinate.resolution_arcminutes, 1.0);
    }
    let school = record(&input, "p295/cengtai/donggou/cengtai-school");
    assert!(school.observation.population.is_none());
    assert!(school.observation.ethnicities.is_empty());
    assert_eq!(
        record(&input, "p290/yongxing/yongping/dapingzi")
            .observation
            .name,
        "大平子"
    );
    assert_eq!(
        record(&input, "p296/cengtai/guanxiang/guanxiangcheng")
            .observation
            .ethnicities,
        ["汉"]
    );
}

#[test]
fn population_totals_remain_distinct_from_constituent_counts() {
    let input = input();
    for (commune, table_page, overview_page, table_pop, overview_pop, households, brigade_sum) in [
        ("yongxing", 290, 289, 7087, 7182, 1452, 7087),
        ("cengtai", 295, 293, 11091, 11091, 2176, 10895),
    ] {
        let table = record(&input, &format!("page-{table_page}/{commune}-commune"));
        let overview = record(
            &input,
            &format!("p{overview_page}/{commune}-commune/population"),
        );
        assert_eq!(table.observation.population, Some(table_pop));
        assert_eq!(overview.observation.population, Some(overview_pop));
        assert_eq!(overview.observation.households, Some(households));
        let sum: u64 = input
            .data
            .records
            .iter()
            .filter(|r| {
                r.observation.parent_id == Some(table.entity_id)
                    && r.observation.rank
                        == Some(atlas_core::assertion::AdministrativeRank::Brigade)
            })
            .filter_map(|r| r.observation.population)
            .sum();
        assert_eq!(sum, brigade_sum);
    }
    for (key, reported, children) in [
        ("p290/yongxing/yongfeng-brigade", 1325, 1397),
        ("p290/yongxing/yongping-brigade", 1624, 1968),
        ("p295/cengtai/guanghui-brigade", 1183, 1283),
        ("p295/cengtai/xiangyang-brigade", 2061, 2065),
        ("p296/cengtai/guanxiang-brigade", 1791, 1800),
    ] {
        let brigade = record(&input, key);
        assert_eq!(brigade.observation.population, Some(reported));
        assert_eq!(
            input
                .data
                .records
                .iter()
                .filter(|r| r.observation.parent_id == Some(brigade.entity_id))
                .filter_map(|r| r.observation.population)
                .sum::<u64>(),
            children
        );
    }
}

#[test]
fn differently_scoped_river_lengths_retain_their_sources_and_shared_identity() {
    let input = input();
    let river = record(&input, "p296/cengtai/wangguan-river");
    assert_eq!(
        river.entity_id,
        record(&input, "p284/sili/wangguan-river").entity_id
    );
    assert_eq!(
        river.entity_id,
        record(&input, "p293/cengtai/wangguan-river/course").entity_id
    );
    assert!(river.observation.coordinate.is_none());
    assert!(river.observation.pinyin.is_none());
    assert!(river.observation.note.as_ref().unwrap().contains("8.5公里"));
    assert!(
        record(&input, "p293/cengtai/wangguan-river/course")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("约8公里")
    );
    assert!(
        record(&input, "p290/yongxing/yongxing-river")
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("25公里")
    );
    assert!(
        record(&input, "p289/yongxing/yongxing-river/course")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("9.5公里")
    );
}

#[cfg(test)]
fn names(input: &LedgerInput, year: i64, selection: TimeSelection) -> BTreeSet<String> {
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
        .filter(|e| e.entity_id == id("cengtai-commune"))
        .flat_map(|e| &e.properties)
        .flat_map(|p| &p.candidates)
        .filter_map(|c| match &c.assertion.value {
            TemporalValue::Name(value) => Some(value.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn commune_names_follow_establishment_renaming_and_restoration_intervals() {
    let input = input();
    for (year, name) in [(1960, "层台公社"), (1978, "红专公社")] {
        assert_eq!(
            names(&input, year, TimeSelection::Certain),
            BTreeSet::from([name.to_owned()]),
            "year {year}"
        );
    }
    assert_eq!(
        names(&input, 1982, TimeSelection::Possible),
        BTreeSet::from(["层台公社".to_owned()])
    );
    assert!(names(&input, 1982, TimeSelection::Certain).is_empty());
    assert!(names(&input, 1957, TimeSelection::Possible).is_empty());
    assert!(names(&input, 1968, TimeSelection::Certain).is_empty());
    assert_eq!(
        names(&input, 1968, TimeSelection::Possible),
        BTreeSet::from(["层台公社".to_owned(), "红专公社".to_owned()])
    );
    assert_eq!(
        record(&input, "p293/cengtai-commune/name-restoration")
            .source_date
            .as_ref()
            .unwrap()
            .text,
        "1981年"
    );
    assert!(
        record(&input, "p289/yongxing-commune/establishment")
            .observation
            .former_names
            .is_empty()
    );
    assert_eq!(
        record(&input, "p289/yongxing/yongxing/ruanqiao/fortress")
            .observation
            .former_names[0]
            .name,
        "阮桥堡"
    );
    for key in [
        "yongxing/shuiyanba",
        "yongxing/yujiaqing-hill",
        "yongxing/guoquanyan",
    ] {
        assert!(
            input
                .data
                .records
                .iter()
                .filter(|r| r.entity_id == id(key))
                .all(|r| r.observation.coordinate.is_none())
        );
    }
}

#[test]
fn the_hongwu_year_disagreement_preserves_both_claims_and_uses_the_calendar_interval() {
    let input = input();
    let source = record(&input, "p293/cengtai/guard/establishment");
    let printed = record(&input, "p293/cengtai/guard/printed-ce-year");
    assert_eq!(
        source.source_date.as_ref().unwrap().text,
        "二十一年（公元1383年）"
    );
    assert_eq!(printed.source_date.as_ref().unwrap().text, "公元1383年");
    let conversion = source
        .source_date
        .as_ref()
        .unwrap()
        .era_conversion
        .as_ref()
        .unwrap();
    assert_eq!(conversion.year, 21);
    assert_eq!(
        conversion.first_day,
        CivilDate {
            year: 1388,
            month: 2,
            day: 8
        }
    );
    assert_eq!(
        conversion.last_day,
        CivilDate {
            year: 1389,
            month: 1,
            day: 27
        }
    );
    let dispute = input
        .data
        .disputes
        .iter()
        .find(|d| d.entity_id == source.entity_id)
        .unwrap();
    assert_eq!(dispute.alternatives, [source.id, printed.id]);
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|e| e.id == dispute.estimate_id)
        .unwrap();
    let EstimateValue::Origin {
        time: OriginTime::Estimated { instant },
    } = &estimate.value
    else {
        panic!("expected converted year bounds")
    };
    assert_eq!(
        instant.earliest,
        conversion.first_day.to_epoch(Calendar::Julian, 0).unwrap()
    );
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, guard_rank) in [(1387, false), (1390, true), (1396, false)] {
        let at = CivilDate {
            year,
            month: 1,
            day: 1,
        }
        .to_epoch(Calendar::Julian, 0)
        .unwrap();
        let slice = timeline.at(at, TimeSelection::Certain).unwrap();
        let present = slice.entities.iter().filter(|e| e.entity_id == source.entity_id)
            .flat_map(|e| &e.properties).flat_map(|p| &p.candidates)
            .any(|c| matches!(&c.assertion.value, TemporalValue::Hierarchy(value) if value.rank == atlas_core::assertion::AdministrativeRank::Guard));
        assert_eq!(present, guard_rank);
    }
}
