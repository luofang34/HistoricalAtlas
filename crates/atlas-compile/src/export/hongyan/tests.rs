use super::*;
use crate::input::load_blocking;
use atlas_boundary::nodal::{NodalBuilder, NodalQuery, NodalScope, RelationKind};
use atlas_core::{
    assertion::TemporalValue,
    calendar::{Calendar, CivilDate},
};
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
fn record<'a>(input: &'a LedgerInput, key: &str) -> &'a Record {
    input
        .data
        .records
        .iter()
        .find(|r| r.key == format!("bijie-gazetteer/{key}"))
        .unwrap()
}

#[test]
fn checked_tables_and_overviews_publish_without_losing_relocated_records() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (311, 12),
        (312, 37),
        (313, 37),
        (314, 6),
        (315, 16),
        (316, 36),
        (317, 33),
        (318, 1),
    ] {
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| r.source_id == "bijie-gazetteer" && r.scan_page == Some(page + 35))
            .collect();
        assert_eq!(rows.len(), count, "page {page}");
        for row in rows {
            assert!(row.checked);
            assert_eq!(
                row.page_or_sheet.as_deref(),
                Some(page.to_string().as_str())
            );
            assert_eq!(
                crate::export::tests::published_record(&output, row),
                serde_json::to_value(row).unwrap()
            );
        }
    }
    for (key, expected) in [
        (
            "page-312/hongyan-commune",
            "5013bb73-4b90-5dad-a7e7-56a361bc2ed6",
        ),
        (
            "page-316/gengguan-commune",
            "af5407bb-2086-58c4-8550-f01fe03577e4",
        ),
        (
            "p313/hongyan/minzu/shanbeihou",
            "0a2f307e-adbf-5cfe-a08a-24b85194cbde",
        ),
        (
            "p316/gengguan/taiping/laofengou",
            "85a7b323-f672-5a33-a6ea-6e66a4d5d705",
        ),
    ] {
        assert_eq!(record(&input, key).id.0.to_string(), expected);
    }
}

#[test]
fn complete_commune_networks_include_cross_page_seats_and_unlocated_features() {
    let input = input();
    for (slug, nodes, settlements, unlocated) in [("hongyan", 82, 68, 3), ("gengguan", 73, 49, 7)] {
        let network = NodalBuilder::new(&input.catalog, &input.data)
            .unwrap()
            .build(NodalQuery {
                entity_id: id(&format!("{slug}-commune")),
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
            unlocated
        );
        let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
        for (kind, count) in [
            (EntityKind::Settlement, settlements),
            (EntityKind::Brigade, 5),
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
            6
        );
    }
    for (key, parent) in [
        ("p313/hongyan/minzu/tianba", "hongyan/minzu-brigade"),
        ("p314/hongyan/fasha/chalin", "hongyan/fasha-brigade"),
        (
            "p317/gengguan/pingzishang/guojiawan",
            "gengguan/pingzishang-brigade",
        ),
    ] {
        assert_eq!(record(&input, key).observation.parent_id, Some(id(parent)));
    }
    assert_eq!(
        record(&input, "p312/hongyan/minzu-brigade")
            .observation
            .seat_id,
        Some(id("hongyan/minzu/tianba"))
    );
}

#[test]
fn original_names_pinyin_and_incomplete_longitude_notation_survive_export() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (key, name, pinyin) in [
        ("page-312/hongyan-commune", "红岩公队", "Hóngyán Gōngshè"),
        ("p313/hongyan/langou/shigelao", "石格佬", "Shígailǎo"),
        ("p313/hongyan/langou-brigade", "兰沟大队", "Nángōu Dàduì"),
        ("p313/hongyan/minzu/guodi", "果底", "Gǒudǐ"),
        ("p316/gengguan/taiping/bangshuitian", "棒水田", "Bàngmùtián"),
        ("p317/gengguan/pingzishang/sunjiatuo", "孙家驼", "Sūnjiātuó"),
        (
            "p317/gengguan/qiaowo-reservoir",
            "荞窝水库",
            "Qiàowō Shuǐkù",
        ),
    ] {
        let published = crate::export::tests::published_record(&output, record(&input, key));
        assert_eq!(published["observation"]["name"], name);
        assert_eq!(published["observation"]["pinyin"], pinyin);
    }
    let row = record(&input, "p312/hongyan/hongyangou/yakoushang");
    let coord = row.observation.coordinate.as_ref().unwrap();
    assert_eq!(coord.longitude_text, "105°31.");
    assert_eq!(coord.latitude_text, "27°36′");
    assert_eq!(coord.resolution_arcminutes, 1.0);
    assert!((coord.point.longitude - (105. + 31. / 60.)).abs() < 1e-12);
}

#[test]
fn source_population_totals_remain_distinct_from_constituent_sums() {
    let input = input();
    for (slug, page, population, households, sum) in [
        ("hongyan", 311, 4889, 1039, 4776),
        ("gengguan", 315, 8164, 1695, 7872),
    ] {
        let commune = record(&input, &format!("p{page}/{slug}-commune/population"));
        assert_eq!(commune.observation.population, Some(population));
        assert_eq!(commune.observation.households, Some(households));
        assert_eq!(
            input
                .data
                .records
                .iter()
                .filter(|r| r.observation.parent_id == Some(commune.entity_id)
                    && r.observation.rank
                        == Some(atlas_core::assertion::AdministrativeRank::Brigade))
                .filter_map(|r| r.observation.population)
                .sum::<u64>(),
            sum
        );
    }
    for (key, reported, children) in [
        ("p312/hongyan/hongyangou-brigade", 1032, 1148),
        ("p312/hongyan/xiaoya-brigade", 915, 978),
        ("p313/hongyan/langou-brigade", 882, 842),
        ("p313/hongyan/fasha-brigade", 721, 624),
        ("p316/gengguan/taiping-brigade", 1705, 2798),
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
fn identical_names_and_positions_do_not_collapse_different_source_features() {
    let input = input();
    for (a, b) in [
        (
            "p312/hongyan/xiaoya/qingshan",
            "p312/hongyan/xiaoya/qingshan-toponym",
        ),
        (
            "p317/gengguan/xiejia/jianyan",
            "p317/gengguan/jianyan-mountain",
        ),
        (
            "p317/gengguan/xiejia/luoshuidong",
            "p317/gengguan/luoshuidong-north-cave",
        ),
    ] {
        let (a, b) = (record(&input, a), record(&input, b));
        assert_ne!(a.entity_id, b.entity_id);
        assert_eq!(a.observation.name, b.observation.name);
        assert_eq!(a.observation.coordinate, b.observation.coordinate);
    }
    let (group, village) = (
        record(&input, "p316/gengguan/taiping/zengjiaping-group"),
        record(&input, "p316/gengguan/taiping/zengjiaping"),
    );
    assert_ne!(group.entity_id, village.entity_id);
    assert_eq!(group.observation.population, Some(651));
    assert_eq!(village.observation.population, Some(186));
    assert_ne!(group.observation.coordinate, village.observation.coordinate);
    let cave = record(&input, "p318/gengguan/luoshuidong-south-cave");
    assert_ne!(
        cave.entity_id,
        record(&input, "p317/gengguan/luoshuidong-north-cave").entity_id
    );
    assert_eq!(cave.observation.note.as_deref(), Some("洞穴、高/深4/700米"));
}

#[test]
fn reservoir_aliases_capacities_and_river_courses_retain_the_source_distinctions() {
    let input = input();
    let table = record(&input, "p317/gengguan/xiaoefeng-reservoir");
    let prose = record(&input, "p315/gengguan/xiaoefeng-reservoir/construction");
    assert_eq!(table.entity_id, prose.entity_id);
    assert_eq!(table.observation.name, "小峨峰水库");
    assert_eq!(prose.observation.name, "谢家水库");
    assert!(table.observation.note.as_ref().unwrap().contains("40万"));
    assert!(prose.verbatim_quote.as_ref().unwrap().contains("60万"));
    let ningjia = record(&input, "p315/gengguan/ningjia-reservoir/construction");
    assert_eq!(ningjia.observation.former_names[0].name, "桥窝水库");
    assert_ne!(
        ningjia.entity_id,
        record(&input, "p317/gengguan/qiaowo-reservoir").entity_id
    );
    for row in [table, prose, ningjia] {
        assert!(row.observation.coordinate.is_none());
    }
    let river = record(&input, "p314/hongyan/wangxiang-river");
    assert_eq!(
        river.observation.note.as_deref(),
        Some("上接太极河，下接普宜小河，境内流长约17公里。")
    );
    assert!(river.observation.coordinate.is_none());
    assert_eq!(
        record(&input, "p315/gengguan/laodao-mountain/elevation")
            .observation
            .name,
        "老道山"
    );
}

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
fn dated_names_distinguish_the_two_communes_and_their_merger() {
    let input = input();
    for (subject, year, name, selection) in [
        (
            "gengguan-commune",
            1960,
            "耿官人民公社",
            TimeSelection::Certain,
        ),
        (
            "gengguan/ningjia-former-commune",
            1960,
            "宁家人民公社",
            TimeSelection::Certain,
        ),
        (
            "gengguan-commune",
            1982,
            "耿官公社",
            TimeSelection::Possible,
        ),
        (
            "hongyan-commune",
            1982,
            "红岩人民公社",
            TimeSelection::Possible,
        ),
    ] {
        assert_eq!(
            names(&input, subject, year, selection),
            BTreeSet::from([name.into()])
        );
    }
    assert!(
        names(
            &input,
            "gengguan/ningjia-former-commune",
            1966,
            TimeSelection::Possible
        )
        .is_empty()
    );
    assert_ne!(
        id("gengguan/ningjia-former-commune"),
        id("gengguan/ningjia-brigade")
    );
    for slug in ["hongyan", "gengguan"] {
        assert!(
            names(
                &input,
                &format!("{slug}-commune"),
                1982,
                TimeSelection::Certain
            )
            .is_empty()
        );
    }
    assert_eq!(
        record(&input, "p315/gengguan-commune/merger")
            .source_date
            .as_ref()
            .unwrap()
            .text,
        "65年"
    );
}
