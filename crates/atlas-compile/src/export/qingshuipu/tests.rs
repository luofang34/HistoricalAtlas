use super::*;
use crate::input::load_blocking;
use atlas_boundary::nodal::{NodalBuilder, NodalQuery, NodalScope, RelationKind};
use atlas_ledger::model::EntityKind;

#[path = "chronology/tests.rs"]
mod chronology;

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
fn complete_reviewed_pages_publish_with_stable_commune_anchor_identities() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (299, 20),
        (300, 1),
        (301, 37),
        (302, 32),
        (303, 5),
        (305, 18),
        (306, 3),
        (307, 37),
        (308, 36),
        (309, 24),
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
            "page-301/qingshuipu-commune",
            "ac14dc9b-8173-5b5f-bbdf-9438c58e7423",
        ),
        (
            "page-307/gaoshanpu-commune",
            "c8de2813-de77-5df8-968a-de175f393da3",
        ),
    ] {
        assert_eq!(record(&input, key).id.0.to_string(), expected);
    }
}

#[test]
fn networks_preserve_every_resident_brigade_and_cross_page_seat() {
    let input = input();
    for (commune, nodes, settlements, brigades, seats) in
        [("qingshuipu", 75, 55, 5, 6), ("gaoshanpu", 99, 72, 9, 10)]
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
            3
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
}

#[test]
fn cross_page_membership_and_the_district_seat_follow_the_reviewed_rows() {
    let input = input();
    for (key, parent) in [
        (
            "p302/qingshuipu/guantian/guantian",
            "qingshuipu/guantian-brigade",
        ),
        (
            "p302/qingshuipu/guantian/xiaocunwuji",
            "qingshuipu/guantian-brigade",
        ),
        (
            "p308/gaoshanpu/shijing/shijing",
            "gaoshanpu/shijing-brigade",
        ),
        (
            "p308/gaoshanpu/shijing/tianjingwo",
            "gaoshanpu/shijing-brigade",
        ),
        (
            "p309/gaoshanpu/nanguan/chengmanyuan",
            "gaoshanpu/nanguan-brigade",
        ),
        (
            "p309/gaoshanpu/nanguan/jiangdi",
            "gaoshanpu/nanguan-brigade",
        ),
    ] {
        assert_eq!(record(&input, key).observation.parent_id, Some(id(parent)));
    }
    for key in [
        "p299/qingshuipu-district/seat",
        "p299/qingshuipu-commune/seat",
    ] {
        assert_eq!(
            record(&input, key).observation.seat_id,
            Some(id("qingshuipu/qingshui/qingshuipu"))
        );
    }
}

#[test]
fn printed_characters_and_pinyin_variants_survive_publication() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (key, name, pinyin) in [
        ("p302/qingshuipu/guantian/yangjiawa", "杨家垇", "Yángjiāwā"),
        ("p307/gaoshanpu/gaoshan/yanwatang", "岩垇塘", "Yánwàtáng"),
        ("p307/gaoshanpu/xiaogou/changwadi", "长垇地", "Chángwàbì"),
        ("p307/gaoshanpu/xujia/yujiashan", "余家山", "Yújiāshār"),
        ("p307/gaoshanpu/xiaogou/xiaogou", "小沟", "Yiǎogōu"),
        (
            "p308/gaoshanpu/lantian-brigade",
            "兰田大队",
            "Nántián Dàduì",
        ),
        ("p301/qingshuipu/houshan/xintian", "新田", "Xīngtián"),
        ("p301/qingshuipu/houshan/shimenpu", "石门铺", "Shímépù"),
        (
            "p302/qingshuipu/lijiaping/chenjiagou",
            "陈家沟",
            "Chéngjiāgōu",
        ),
        ("p303/qingshuipu/laohuwo-mountain", "老虎窝", "Lǎofǔwō"),
        (
            "p303/qingshuipu/dushantang-reservoir",
            "独山塘水库",
            "Dúshānttángshuǐkù",
        ),
        ("p302/qingshuipu/jifengbao-mountain", "祭风包", "Jìfóngbāo"),
        ("p309/gaoshanpu/jifengbao-mountain", "祭风包", "Jìfōngbāo"),
    ] {
        let published = crate::export::tests::published_record(&output, record(&input, key));
        assert_eq!(published["observation"]["name"], name);
        assert_eq!(published["observation"]["pinyin"], pinyin);
    }
}

#[test]
fn printed_totals_do_not_replace_disagreeing_constituent_populations() {
    let input = input();
    for (commune, page, overview, population, households, total) in [
        ("qingshuipu", 301, 299, 8787, 1895, 7573),
        ("gaoshanpu", 307, 305, 9669, 1970, 17260),
    ] {
        let table = record(&input, &format!("page-{page}/{commune}-commune"));
        let prose = record(&input, &format!("p{overview}/{commune}-commune/population"));
        assert_eq!(table.observation.population, Some(population));
        assert_eq!(prose.observation.population, Some(population));
        assert_eq!(prose.observation.households, Some(households));
        assert_eq!(
            input
                .data
                .records
                .iter()
                .filter(|r| r.observation.parent_id == Some(table.entity_id)
                    && r.observation.rank
                        == Some(atlas_core::assertion::AdministrativeRank::Brigade))
                .filter_map(|r| r.observation.population)
                .sum::<u64>(),
            total
        );
    }
    for (key, reported, children) in [
        ("p307/gaoshanpu/daping-brigade", 901, 2628),
        ("p307/gaoshanpu/xiaogou-brigade", 1044, 1157),
        ("p307/gaoshanpu/shijing-brigade", 791, 678),
        ("p308/gaoshanpu/nanguan-brigade", 9415, 1563),
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
    assert_eq!(
        record(&input, "p307/gaoshanpu/daping/luojiaping")
            .observation
            .population,
        Some(1803)
    );
    assert_eq!(
        record(&input, "p307/gaoshanpu/xiaogou/changwadi")
            .observation
            .parent_id,
        Some(id("gaoshanpu/xiaogou-brigade"))
    );
}

#[test]
fn same_coordinates_names_and_grouped_settlements_retain_separate_identities() {
    let input = input();
    for (a, b) in [
        (
            "p307/gaoshanpu/daping/dapingzi",
            "p307/gaoshanpu/xiaogou/dapingzi",
        ),
        (
            "p308/gaoshanpu/lantian/wangjiashan",
            "p309/gaoshanpu/wangjiashan-mountain",
        ),
    ] {
        let (a, b) = (record(&input, a), record(&input, b));
        assert_ne!(a.entity_id, b.entity_id);
        assert_eq!(a.observation.coordinate, b.observation.coordinate);
    }
    let (place, ruin) = (
        record(&input, "p301/qingshuipu/dongshan/xinwenge"),
        record(&input, "p303/qingshuipu/xinwenge-site"),
    );
    assert_eq!(place.observation.name, ruin.observation.name);
    assert_ne!(place.entity_id, ruin.entity_id);
    assert_ne!(place.observation.coordinate, ruin.observation.coordinate);
    let group = record(&input, "p308/gaoshanpu/lantian/chayegou");
    assert!(group.observation.population.is_none());
    assert_eq!(group.observation.ethnicities, ["汉"]);
    assert_eq!(
        group.observation.note.as_deref(),
        Some("片村包括上、下茶峰")
    );
    assert!(
        input
            .data
            .entities
            .iter()
            .any(|e| e.id == group.entity_id && e.kind == EntityKind::Settlement)
    );
    for key in ["shangchafeng", "xiachafeng"] {
        assert_ne!(
            group.entity_id,
            record(&input, &format!("p308/gaoshanpu/lantian/{key}")).entity_id
        );
    }
    let blank = record(&input, "p301/qingshuipu/houshan/dongqingshu");
    assert!(blank.observation.population.is_none());
    assert!(blank.observation.ethnicities.is_empty());
}

#[test]
fn outlying_coordinates_remain_observations_and_do_not_seed_the_partition() {
    let input = input();
    let recipe = &input.derivations["bijie-communes-terrain"];
    for (key, longitude, latitude, excluded) in [
        (
            "p301/qingshuipu/houshan/dongqingshu",
            "105°31′",
            "27°49′",
            true,
        ),
        (
            "p303/qingshuipu/laohuwo-mountain",
            "103°34′",
            "27°38′",
            false,
        ),
        (
            "p309/gaoshanpu/nanguan/chengmanyuan",
            "105°35′",
            "27°43′",
            true,
        ),
        ("p308/gaoshanpu/nanguan/fengyan", "105°35′", "27°43′", true),
    ] {
        let row = record(&input, key);
        let coordinate = row.observation.coordinate.as_ref().unwrap();
        assert_eq!(coordinate.longitude_text, longitude);
        assert_eq!(coordinate.latitude_text, latitude);
        assert_eq!(coordinate.resolution_arcminutes, 1.0);
        assert_eq!(
            recipe.excluded.iter().any(|e| e.record_id == row.id),
            excluded
        );
        assert!(!recipe.units.iter().flat_map(|u|&u.anchors).any(|a|
            matches!(a,crate::derivation::AnchorReference::Record{record_id} if *record_id==row.id)));
    }
    let commune = record(&input, "page-301/qingshuipu-commune");
    let seat = record(&input, "p301/qingshuipu/qingshui/qingshuipu");
    assert_eq!(commune.observation.seat_id, Some(seat.entity_id));
    assert_eq!(
        commune
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .longitude_text,
        "105°31′"
    );
    assert_eq!(
        seat.observation.coordinate.as_ref().unwrap().longitude_text,
        "105°33′"
    );
}

#[test]
fn rivers_reservoirs_and_bridge_annotations_keep_their_printed_scope() {
    let input = input();
    let river = record(&input, "p309/chishui-river");
    assert_eq!(river.entity_id, id("chishui-river"));
    assert!(river.observation.coordinate.is_none());
    assert_eq!(
        river.observation.note.as_deref(),
        Some("上接耿官下接普宜小河、公子河约河流12公里")
    );
    assert_eq!(
        record(&input, "p309/gaoshanpu/chishui-river-bridge")
            .observation
            .note
            .as_deref(),
        Some("公路桥；113—4 / 10")
    );
    let reservoir = record(&input, "p303/qingshuipu/yinchangyan-reservoir");
    assert!(reservoir.observation.coordinate.is_none());
    assert_eq!(
        reservoir.observation.note.as_deref(),
        Some("水库、库容18万立方米")
    );
    let station = record(&input, "p308/gaoshanpu/lantian/xulong-power-station");
    assert_eq!(station.observation.name, "须龙电站");
    assert!(
        record(&input, "p305/gaoshanpu-commune/waterworks")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("龙须电站两个")
    );
    assert!(
        record(&input, "p299/qingshuipu/longtan-mountain/elevation")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("最高处")
    );
    assert_eq!(
        record(&input, "p302/qingshuipu/jifengbao-mountain")
            .observation
            .note
            .as_deref(),
        Some("山峰、海拔1681.8米")
    );
}
