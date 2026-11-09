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
fn all_reviewed_rows_publish_with_the_existing_commune_record_identity() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (319, 24),
        (320, 2),
        (321, 37),
        (322, 37),
        (323, 37),
        (324, 36),
        (325, 23),
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
    assert_eq!(
        record(&input, "page-321/shengji-commune").id.0.to_string(),
        "6e6bbe50-3302-5ce0-a7cf-ab031c2e9254"
    );
}

#[test]
fn source_network_keeps_every_village_brigade_and_cross_page_seat() {
    let input = input();
    let network = NodalBuilder::new(&input.catalog, &input.data)
        .unwrap()
        .build(NodalQuery {
            entity_id: id("shengji-commune"),
            scope: NodalScope::Source {
                source_id: "bijie-gazetteer".into(),
            },
        })
        .unwrap();
    assert_eq!(network.nodes.len(), 180);
    assert_eq!(
        network
            .nodes
            .iter()
            .filter(|n| n.anchors.is_empty())
            .count(),
        19
    );
    let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
    for (kind, count) in [(EntityKind::Settlement, 113), (EntityKind::Brigade, 12)] {
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
        13
    );
    for (key, parent) in [
        (
            "p322/shengji/zhenjiang/wangjiayuan",
            "shengji/zhenjiang-brigade",
        ),
        ("p323/shengji/xianghe/lugou", "shengji/xianghe-brigade"),
        ("p324/shengji/gaoliu/wangjiashan", "shengji/gaoliu-brigade"),
    ] {
        assert_eq!(record(&input, key).observation.parent_id, Some(id(parent)));
    }
    assert_eq!(
        record(&input, "p321/shengji/zhenjiang-brigade")
            .observation
            .seat_id,
        Some(id("shengji/zhenjiang/zhenjiangsi"))
    );
    assert_eq!(
        record(&input, "p322/shengji/xianghe-brigade")
            .observation
            .seat_id,
        Some(id("shengji/xianghe/xiangshugeng"))
    );
}

#[test]
fn population_claims_retain_their_disagreements_with_constituent_sums() {
    let input = input();
    let table = record(&input, "page-321/shengji-commune");
    let prose = record(&input, "p319/shengji-commune/population");
    assert_eq!(table.observation.population, Some(1486));
    assert_eq!(prose.observation.population, Some(14886));
    assert_eq!(prose.observation.households, Some(3092));
    assert!(
        prose
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("彝族479人，苗族804人")
    );
    let brigades: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|r| {
            r.observation.parent_id == Some(table.entity_id)
                && r.observation.rank == Some(atlas_core::assertion::AdministrativeRank::Brigade)
        })
        .collect();
    assert_eq!(
        brigades
            .iter()
            .filter_map(|r| r.observation.population)
            .sum::<u64>(),
        14237
    );
    for (key, reported, children) in [
        ("p321/shengji/zhenjiang-brigade", 935, 1043),
        ("p322/shengji/xianghe-brigade", 1400, 1449),
        ("p323/shengji/hetaoping-brigade", 775, 777),
        ("p323/shengji/gaoliu-brigade", 1166, 1058),
        ("p324/shengji/gebu-brigade", 971, 921),
        ("p324/shengji/dayantang-brigade", 2210, 2476),
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
fn printed_coordinates_and_spellings_survive_export_without_becoming_boundary_seeds() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (key, name, pinyin) in [
        ("p321/shengji/zhenjiang/hantangwa", "韩塘垇", "Hántángwà"),
        ("p322/shengji/xianghe/macaowa", "马槽垇", "Mǎcwá"),
        (
            "p323/shengji/xianghe/zhangjiachong",
            "张家冲",
            "Zhāngjiāehōng",
        ),
        ("p323/shengji/xianghe/wujiashan", "吴家山", "Wójiāshān"),
        ("p325/shengji/gebu-canal", "各哺大沟", "Gépù Dàgōu"),
    ] {
        let row = crate::export::tests::published_record(&output, record(&input, key));
        assert_eq!(row["observation"]["name"], name);
        assert_eq!(row["observation"]["pinyin"], pinyin);
    }
    let recipe = &input.derivations["bijie-communes-terrain"];
    for (key, longitude) in [
        ("p324/shengji/gebu/changwanzi", "103°26′"),
        ("p322/shengji/qingping/dayakou", "105°23′"),
    ] {
        let row = record(&input, key);
        assert_eq!(
            row.observation.coordinate.as_ref().unwrap().longitude_text,
            longitude
        );
        assert!(recipe.excluded.iter().any(|e| e.record_id == row.id));
        assert!(!recipe.units.iter().flat_map(|u| &u.anchors).any(|a| matches!(a, crate::derivation::AnchorReference::Record { record_id } if *record_id == row.id)));
    }
    assert!(
        record(&input, "p319/shengji-commune/extent")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("105°′32′")
    );
    assert!(
        record(&input, "p319/shengji-commune/population")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("葛浦十二个大队")
    );
}

#[test]
fn repeated_names_keep_separate_settlements_groups_caves_and_temples() {
    let input = input();
    for (a, b) in [
        ("p324/shengji/gebu/haizi", "p324/shengji/gebu/haizi-group"),
        (
            "p323/shengji/duanping/luoshuidong",
            "p325/shengji/luoshuidong-cave",
        ),
        (
            "p321/shengji/shengji/zhaozishan",
            "p325/shengji/zhaozishan-mountain",
        ),
        (
            "p325/shengji/longshen-zhenjiang-temple",
            "p325/shengji/longshen-west-temple",
        ),
        (
            "p322/shengji/zhenjiang/zhenjiangsi",
            "p325/shengji/longshen-zhenjiang-temple",
        ),
    ] {
        assert_ne!(record(&input, a).entity_id, record(&input, b).entity_id);
    }
    let group = record(&input, "p324/shengji/gebu/haizi-group");
    let village = record(&input, "p324/shengji/gebu/haizi");
    assert_eq!(group.observation.population, None);
    assert_eq!(village.observation.population, Some(152));
    assert_ne!(group.observation.coordinate, village.observation.coordinate);
    assert_eq!(
        group.observation.note.as_deref(),
        Some("包括石龙沟、毛栗树丫口海子")
    );
    assert_eq!(
        record(&input, "p325/shengji/longshen-zhenjiang-temple")
            .observation
            .note
            .as_deref(),
        Some("（镇江寺）现废")
    );
}

#[test]
fn waterworks_keep_literal_capacities_and_unlocated_courses() {
    let input = input();
    for (slug, capacity, located) in [
        ("luoquanyan", "23.3立方米", true),
        ("yinchang", "21.3立方米", false),
        ("xiaoheping", "81立方米", false),
    ] {
        let row = record(&input, &format!("p325/shengji/{slug}-reservoir"));
        assert_eq!(
            row.observation.note.as_deref(),
            Some(format!("水库，库容{capacity}").as_str())
        );
        assert_eq!(row.observation.coordinate.is_some(), located);
    }
    let prose = record(&input, "p319/shengji/luoquanyan-reservoir/construction");
    assert_eq!(
        prose.entity_id,
        record(&input, "p325/shengji/luoquanyan-reservoir").entity_id
    );
    assert!(
        prose
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("十万方以上水库十个")
    );
    assert_ne!(
        id("shengji/yinchang-reservoir"),
        id("shengji/yinchanggou-reservoir")
    );
    for slug in [
        "xishan",
        "gaoliu",
        "huangdong",
        "hetao",
        "xianghe",
        "gebu",
        "zhenjiang",
    ] {
        let row = record(&input, &format!("p325/shengji/{slug}-canal"));
        assert!(row.observation.coordinate.is_none());
        assert!(row.observation.population.is_none());
        assert!(row.observation.note.as_ref().unwrap().contains("灌田"));
    }
    let river = record(&input, "p319/shengji/shilonggou-stream/course");
    assert!(river.observation.coordinate.is_none());
    assert!(
        river
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("向东北流入赤水河")
    );
}

#[test]
fn commune_and_reservoir_origins_follow_the_cited_year_ranges() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (subject, year, selection, count) in [
        ("shengji-commune", 1957, TimeSelection::Possible, 0),
        ("shengji-commune", 1959, TimeSelection::Certain, 1),
        (
            "shengji/luoquanyan-reservoir",
            1964,
            TimeSelection::Possible,
            0,
        ),
        (
            "shengji/luoquanyan-reservoir",
            1966,
            TimeSelection::Possible,
            1,
        ),
        (
            "shengji/luoquanyan-reservoir",
            1966,
            TimeSelection::Certain,
            0,
        ),
        (
            "shengji/luoquanyan-reservoir",
            1979,
            TimeSelection::Certain,
            1,
        ),
        (
            "shengji/yinchanggou-reservoir",
            1979,
            TimeSelection::Certain,
            1,
        ),
    ] {
        let at = CivilDate {
            year,
            month: 8,
            day: 15,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, selection).unwrap();
        assert_eq!(
            slice
                .entities
                .iter()
                .filter(|e| e.entity_id == id(subject))
                .flat_map(|e| &e.origins)
                .count(),
            count,
            "{subject} {year}"
        );
    }
}

#[test]
fn commune_name_appears_in_the_possible_timeline_after_its_establishment() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    let at = CivilDate {
        year: 1982,
        month: 8,
        day: 15,
    }
    .to_epoch(Calendar::Gregorian, 0)
    .unwrap();
    let slice = timeline.at(at, TimeSelection::Possible).unwrap();
    let commune = slice
        .entities
        .iter()
        .find(|e| e.entity_id == id("shengji-commune"))
        .unwrap();
    assert!(
        commune.properties.iter().flat_map(|p| &p.candidates).any(
            |c| matches!(&c.assertion.value, TemporalValue::Name(n) if n.text == "生机人民公社")
        )
    );
}
