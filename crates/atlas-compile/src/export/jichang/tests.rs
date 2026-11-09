use super::*;
use crate::input::load_blocking;
use atlas_boundary::nodal::{NodalBuilder, NodalQuery, NodalScope, RelationKind};
use atlas_core::calendar::{Calendar, CivilDate};
use atlas_ledger::{
    model::EntityKind,
    timeline::{TimeSelection, Timeline},
};

#[path = "sili_zongji/tests.rs"]
mod sili_zongji;

#[path = "yongxing_cengtai/tests.rs"]
mod yongxing_cengtai;

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
fn reviewed_pages_round_trip_and_keep_commune_and_neighbor_identities() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (273, 13),
        (274, 1),
        (275, 34),
        (276, 27),
        (277, 10),
        (278, 36),
        (279, 35),
        (280, 1),
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
            "page-275/xiaoji-commune",
            "0933e472-1d60-5269-b4c2-f6b93922fb40",
        ),
        (
            "page-278/daji-commune",
            "5ee26a82-80e0-584d-98e0-7b71d1f435ae",
        ),
        (
            "p278/daji/longling-brigade",
            "5dcde592-9039-598c-a2e7-e668713e7568",
        ),
        (
            "p279/daji/longling/yuduo",
            "804aa794-fc71-59d2-96de-4f760e02b780",
        ),
        (
            "p279/daji/longling/kuangjiaqing",
            "9d729200-90f7-5d3f-b9bc-90cb42d81fda",
        ),
    ] {
        assert_eq!(record(&input, key).id.0.to_string(), expected);
    }
}

#[test]
fn networks_resolve_cross_page_membership_seats_and_unlocated_features() {
    let input = input();
    for (commune, nodes, settlements, brigades, seats) in
        [("xiaoji", 61, 46, 7, 8), ("daji", 72, 59, 6, 7)]
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
            1
        );
        let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
        for (kind, count) in [
            (EntityKind::Settlement, settlements),
            (EntityKind::Brigade, brigades),
            (EntityKind::Facility, 3),
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
    for (key, parent) in [
        ("p279/daji/longdongwan/xiachang", "daji/longdongwan-brigade"),
        (
            "p279/daji/longdongwan/xiachang-road-station",
            "daji/longdongwan-brigade",
        ),
        ("p279/daji/longling/heshangpo", "daji/longling-brigade"),
        ("p276/xiaoji/jiping/bailaogou", "xiaoji/jiping-brigade"),
    ] {
        assert_eq!(record(&input, key).observation.parent_id, Some(id(parent)));
    }
    let seat = Some(id("xiaoji/jixin/xiaojichang"));
    for key in [
        "p275/jichang-district",
        "page-275/xiaoji-commune",
        "p275/xiaoji/jixin-brigade",
    ] {
        assert_eq!(record(&input, key).observation.seat_id, seat);
    }
}

#[test]
fn printed_variants_and_empty_fields_remain_distinct_from_interpretations() {
    let input = input();
    for key in [
        "p278/daji/daji/longjiagou",
        "p276/xiaoji/lizi/zhangjiapingzi",
    ] {
        let row = record(&input, key);
        assert!(row.observation.population.is_none());
        assert!(
            input
                .data
                .entities
                .iter()
                .any(|e| e.id == row.entity_id && e.kind == EntityKind::Settlement)
        );
    }
    for (key, pinyin) in [
        ("p278/daji/daji/laojie", "Jǎojiē"),
        ("p279/daji/longling/yuduo", "Yǔdǔo"),
        ("p279/daji/gaoxing/houziqiao", "Huózǐqiáo"),
        ("p279/gongzihe/houziqiao-power-station", "Hóuzǐqiáopiànzhàn"),
        ("p276/xiaoji/luogucaipo-mountain", "Luóqǔcàipō"),
    ] {
        assert_eq!(
            record(&input, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    let table = record(&input, "page-278/daji-commune");
    assert_eq!(table.observation.population, Some(9512));
    assert_eq!(table.observation.ethnicities, ["汉", "苗", "南京"]);
    assert_eq!(
        record(&input, "p277/daji-commune/population")
            .observation
            .population,
        Some(9682)
    );
    assert!(
        record(&input, "p273/xiaoji-commune/population")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("吉兴")
    );
    assert_eq!(
        record(&input, "p275/xiaoji/jixin-brigade").observation.name,
        "吉新大队"
    );
    let a = record(&input, "p275/xiaoji/jixin/wangjiawan");
    let b = record(&input, "p275/xiaoji/silun/wangjiawan");
    assert_eq!(a.observation.name, b.observation.name);
    assert_ne!(a.entity_id, b.entity_id);
    assert_ne!(a.observation.parent_id, b.observation.parent_id);
    assert_ne!(a.observation.coordinate, b.observation.coordinate);
}

#[test]
fn outlying_printed_longitude_is_published_without_seeding_a_boundary() {
    let input = input();
    let row = record(&input, "p275/xiaoji/jifeng/damuchang");
    let coordinate = row.observation.coordinate.as_ref().unwrap();
    assert_eq!(coordinate.longitude_text, "103°32′");
    assert!((coordinate.point.longitude - (103. + 32. / 60.)).abs() < 1e-12);
    let recipe = &input.derivations["bijie-communes-terrain"];
    assert!(recipe.excluded.iter().any(|e| e.record_id == row.id));
    assert!(!recipe.units.iter().flat_map(|u| &u.anchors).any(|a| matches!(a, crate::derivation::AnchorReference::Record { record_id } if *record_id == row.id)));
}

#[test]
fn daji_continuation_page_retains_the_bridge_and_its_printed_notation() {
    let input = input();
    let bridge = record(&input, "p280/daji/daji-bridge");
    assert_eq!(bridge.scan_page, Some(315));
    assert_eq!(bridge.observation.parent_id, Some(id("daji-commune")));
    assert!(bridge.observation.population.is_none());
    assert_eq!(bridge.observation.note.as_deref(), Some("公路桥；5—4 / 5"));
    assert_eq!(
        bridge.observation.coordinate,
        record(&input, "page-278/daji-commune")
            .observation
            .coordinate
    );
    assert!(
        input
            .data
            .entities
            .iter()
            .any(|e| e.id == bridge.entity_id && e.kind == EntityKind::Bridge)
    );
}

#[test]
fn neighboring_brigade_evidence_keeps_shared_coordinate_cells_ambiguous() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    let recipe = &input.derivations["bijie-communes-terrain"];
    for (key, parent, longitude, latitude, excluded) in [
        (
            "p284/sili/lianfeng/pingzishang",
            "sili/lianfeng-brigade",
            "105°30′",
            "27°33′",
            true,
        ),
        (
            "p284/sili/yanwan/shiyakou",
            "sili/yanwan-brigade",
            "105°32′",
            "27°34′",
            true,
        ),
        (
            "p312/hongyan/xiaoya/xiaoyudong",
            "hongyan/xiaoya-brigade",
            "105°32′",
            "27°34′",
            true,
        ),
        (
            "p312/hongyan/xiaoya/hejiatian",
            "hongyan/xiaoya-brigade",
            "105°32′",
            "27°35′",
            false,
        ),
        (
            "p313/hongyan/fasha/dayakou",
            "hongyan/fasha-brigade",
            "105°33′",
            "27°35′",
            true,
        ),
    ] {
        let row = record(&input, key);
        assert!(row.checked);
        assert_eq!(row.observation.parent_id, Some(id(parent)));
        let coordinate = row.observation.coordinate.as_ref().unwrap();
        assert_eq!(coordinate.longitude_text, longitude);
        assert_eq!(coordinate.latitude_text, latitude);
        assert_eq!(
            recipe.excluded.iter().any(|e| e.record_id == row.id),
            excluded
        );
        assert_eq!(
            crate::export::tests::published_record(&output, row),
            serde_json::to_value(row).unwrap()
        );
    }
    for (key, seat) in [
        ("p284/sili/lianfeng-brigade", "sili/lianfeng/damiaozhai"),
        ("p284/sili/yanwan-brigade", "sili/yanwan/zengjiayakou"),
        ("p312/hongyan/xiaoya-brigade", "hongyan/xiaoya/bajiaotang"),
        ("p313/hongyan/fasha-brigade", "hongyan/fasha/fasha"),
    ] {
        assert_eq!(record(&input, key).observation.seat_id, Some(id(seat)));
    }
}

#[cfg(test)]
fn names(
    input: &LedgerInput,
    entity: &str,
    year: i64,
    selection: TimeSelection,
) -> BTreeSet<String> {
    let at = CivilDate {
        year,
        month: 6,
        day: 1,
    }
    .to_epoch(Calendar::Gregorian, 0)
    .unwrap();
    Timeline::new(&input.catalog, &input.data)
        .unwrap()
        .at(at, selection)
        .unwrap()
        .entities
        .iter()
        .filter(|e| e.entity_id == id(entity))
        .flat_map(|e| &e.properties)
        .flat_map(|p| &p.candidates)
        .filter_map(|c| match &c.assertion.value {
            TemporalValue::Name(n) => Some(n.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn district_and_commune_names_follow_the_cited_transition_years() {
    let input = input();
    for (entity, year, name) in [
        ("jichang-district", 1955, "九区"),
        ("jichang-district", 1957, "吉场区"),
        ("daji-commune", 1959, "团结公社"),
        ("daji-commune", 1961, "大吉公社"),
        ("xiaoji-commune", 1959, "小吉公社"),
    ] {
        assert_eq!(
            names(&input, entity, year, TimeSelection::Nominal),
            BTreeSet::from([name.into()])
        );
    }
    for (entity, year, former, current) in [
        ("jichang-district", 1956, "九区", "吉场区"),
        ("daji-commune", 1960, "团结公社", "大吉公社"),
    ] {
        assert_eq!(
            names(&input, entity, year, TimeSelection::Possible),
            BTreeSet::from([former.into(), current.into()])
        );
        assert!(names(&input, entity, year, TimeSelection::Certain).is_empty());
    }
    for (key, text) in [
        ("p273/jichang-district/renaming", "56年"),
        ("p277/daji-commune/establishment", "58年"),
        ("p277/daji-commune/renaming", "60年"),
    ] {
        let date = record(&input, key).source_date.as_ref().unwrap();
        assert_eq!(date.text, text);
        assert!(date.conversion_basis.is_some());
    }
}

#[test]
fn station_construction_completion_and_nearby_village_keep_separate_meanings() {
    let input = input();
    let construction = record(&input, "p265/gongzihe/houziqiao-power-station/construction");
    let completion = record(&input, "p277/gongzihe/houziqiao-power-station/completion");
    let station = record(&input, "p279/gongzihe/houziqiao-power-station");
    let village = record(&input, "p279/daji/gaoxing/houziqiao");
    assert_eq!(construction.entity_id, completion.entity_id);
    assert_eq!(station.entity_id, completion.entity_id);
    assert_ne!(station.entity_id, village.entity_id);
    assert_eq!(
        station.observation.coordinate,
        village.observation.coordinate
    );
    assert_eq!(station.observation.parent_id, Some(id("daji-commune")));
    assert_eq!(construction.source_date.as_ref().unwrap().text, "1968年");
    assert_eq!(completion.source_date.as_ref().unwrap().text, "1978年");
    assert!(
        completion
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("400千瓦")
    );
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|e| {
            e.entity_id == station.entity_id && matches!(e.value, EstimateValue::Origin { .. })
        })
        .unwrap();
    assert_eq!(estimate.basis, [completion.id]);
    for (year, count) in [(1969, 0), (1977, 0), (1979, 1)] {
        let at = CivilDate {
            year,
            month: 1,
            day: 1,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = Timeline::new(&input.catalog, &input.data)
            .unwrap()
            .at(at, TimeSelection::Certain)
            .unwrap();
        assert_eq!(
            slice
                .entities
                .iter()
                .find(|e| e.entity_id == station.entity_id)
                .map(|e| e.origins.len())
                .unwrap_or_default(),
            count
        );
    }
}
