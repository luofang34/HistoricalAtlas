use super::*;
use crate::input::load_blocking;
use atlas_boundary::nodal::{NodalBuilder, NodalQuery, NodalScope, RelationKind};
use atlas_core::calendar::{Calendar, CivilDate};
use atlas_ledger::{
    model::EntityKind,
    timeline::{TimeSelection, Timeline},
};

#[path = "ashi_xiaohe/tests.rs"]
mod ashi_xiaohe;
#[path = "bozha_liuwan/tests.rs"]
mod bozha_liuwan;
#[path = "gongzihe_gala/tests.rs"]
mod gongzihe_gala;
#[path = "huoma_shatian/tests.rs"]
mod huoma_shatian;

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

#[test]
fn puyi_and_ledu_pages_publish_reviewed_fields_and_retain_anchor_identities() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [
        (237, 11),
        (238, 36),
        (239, 12),
        (241, 7),
        (242, 32),
        (243, 25),
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
    for (key, expected, population) in [
        (
            "page-238/puyi-commune",
            "31b184c3-9558-5fc6-b4a4-dee344e0e065",
            5501,
        ),
        (
            "page-242/ledu-commune",
            "23d4534b-b853-55eb-b2b9-dc3f6705cd70",
            4761,
        ),
    ] {
        let commune = input
            .data
            .records
            .iter()
            .find(|r| r.key == format!("bijie-gazetteer/{key}"))
            .unwrap();
        assert_eq!(commune.id.0.to_string(), expected);
        assert_eq!(commune.observation.population, Some(population));
        assert_eq!(commune.observation.parent_id, Some(id("puyi-district")));
    }
    for (page, key, pinyin) in [
        (238, "puyi/banpo/banpo", "Bàpō"),
        (238, "puyi/yanjiao/yanjiaozhai", "Yánjiǎzhài"),
        (238, "puyi/yanjiao/xuejiazhai", "Xújiāzhài"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    let blank = record(&input, 238, "puyi/laojie/yangjiazhai");
    assert!(blank.observation.population.is_none());
    assert!(blank.observation.ethnicities.is_empty());
    for key in ["ledu/ledu/dahuangpo", "ledu/dawanzi/huajiaoshan"] {
        let row = record(&input, 242, key);
        assert!(row.observation.population.is_some());
        assert!(row.observation.ethnicities.is_empty());
    }
}

#[test]
fn commune_networks_distinguish_residents_schools_and_the_two_ledu_seat_claims() {
    let input = input();
    for (commune, residents, brigades, nodes, seats) in
        [("puyi", 40, 5, 47, 6), ("ledu", 48, 6, 57, 8)]
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
        let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
        for (kind, count) in [
            (EntityKind::Settlement, residents),
            (EntityKind::Brigade, brigades),
            (EntityKind::Facility, 1),
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
        assert!(network.nodes.iter().all(|n| !n.anchors.is_empty()));
        let links: BTreeSet<_> = network
            .links
            .iter()
            .filter(|l| l.relation == RelationKind::Seat)
            .map(|l| (l.from, l.to))
            .collect();
        assert_eq!(links.len(), seats);
        if commune == "ledu" {
            for seat in ["taiping", "lantianwan"] {
                assert!(
                    links.contains(&(id("ledu-commune"), id(&format!("ledu/taipingchang/{seat}"))))
                );
            }
        }
    }
    for (page, key, population) in [
        (238, "puyi/central-school", 1020),
        (243, "ledu/primary-school", 492),
    ] {
        let row = record(&input, page, key);
        assert_eq!(row.observation.population, Some(population));
        assert!(
            input
                .data
                .entities
                .iter()
                .any(|e| e.id == row.entity_id && e.kind == EntityKind::Facility)
        );
    }
    let taiping = record(&input, 242, "ledu/taipingchang/taiping");
    let lantianwan = record(&input, 242, "ledu/taipingchang/lantianwan");
    assert_eq!(
        taiping.observation.coordinate,
        lantianwan.observation.coordinate
    );
    assert_ne!(taiping.entity_id, lantianwan.entity_id);
    assert!(lantianwan.observation.population.is_none());
    assert_eq!(
        lantianwan.observation.note.as_deref(),
        Some("片村，包括：上寨、长环、槽子头、田埂")
    );
}

#[test]
fn same_named_villages_keep_brigade_membership_and_source_demographics() {
    let input = input();
    for (name, a, b) in [
        (
            "上寨",
            (242, "ledu/taipingchang/shangzhai", 43),
            (243, "ledu/tianwei/shangzhai", 139),
        ),
        (
            "坪子上",
            (242, "ledu/ledu/pingzishang", 50),
            (243, "ledu/lengshui/pingzishang", 80),
        ),
        (
            "岩头上",
            (238, "puyi/xinjie/yantoushang", 90),
            (239, "puyi/qingjiao/yantoushang", 58),
        ),
    ] {
        let first = record(&input, a.0, a.1);
        let second = record(&input, b.0, b.1);
        assert_eq!(first.observation.name, name);
        assert_eq!(second.observation.name, name);
        assert_eq!(first.observation.population, Some(a.2));
        assert_eq!(second.observation.population, Some(b.2));
        assert_ne!(first.entity_id, second.entity_id);
        assert_ne!(first.observation.parent_id, second.observation.parent_id);
        assert_ne!(first.observation.coordinate, second.observation.coordinate);
    }
    let group = record(&input, 243, "ledu/lengshui/houzigou");
    assert!(group.observation.population.is_none());
    assert_eq!(group.observation.note.as_deref(), Some("包括坪子上、上沟"));
    assert_eq!(
        record(&input, 242, "ledu/taipingchang/huangdu")
            .observation
            .ethnicities,
        ["汉", "京", "羿"]
    );
    let district = record(&input, 238, "puyi-district");
    assert!(district.observation.coordinate.is_none());
    assert_eq!(district.observation.parent_id, Some(id("bijie-county")));
}

#[test]
fn commune_origins_preserve_year_precision_without_dating_the_district_rename() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, key, expected) in [
        (1949, "puyi-district", 0),
        (1951, "puyi-district", 1),
        (1957, "puyi-commune", 0),
        (1959, "puyi-commune", 1),
        (1957, "ledu-commune", 0),
        (1959, "ledu-commune", 1),
    ] {
        let at = CivilDate {
            year,
            month: 8,
            day: 15,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, TimeSelection::Certain).unwrap();
        assert_eq!(
            slice
                .entities
                .iter()
                .find(|e| e.entity_id == id(key))
                .map(|e| e.origins.len())
                .unwrap_or_default(),
            expected
        );
    }
    let rename = record(&input, 237, "puyi-district/name");
    assert!(rename.source_date.is_none());
    assert_eq!(rename.observation.former_names[0].name, "第八区");
    assert!(rename.observation.former_names[0].validity.is_none());
    let former = record(&input, 241, "ledu/ledu/ledu/name");
    assert_eq!(former.observation.former_names[0].name, "裸都");
    assert!(former.observation.former_names[0].source_date.is_none());
    let puyi = record(&input, 237, "puyi/laojie/puyi/name");
    assert_eq!(puyi.observation.name, "普泥");
    assert_eq!(
        puyi.entity_id,
        record(&input, 238, "puyi/laojie/puyi").entity_id
    );
}

#[test]
fn ambiguous_and_external_coordinates_remain_published_but_do_not_seed_boundaries() {
    let input = input();
    let request = &input.derivations["bijie-communes-terrain"];
    for (page, key) in [
        (238, "puyi/yanjiao/zhongxinchang"),
        (238, "puyi/laojie/yangjiazhai"),
        (239, "puyi/qingjiao/huangjiatu"),
        (242, "ledu/ledu/xialedu"),
        (242, "ledu/tianwei/tianweifa"),
        (243, "ledu/yakoushang/jinjiazhai"),
        (238, "puyi/xinjie/taojianao"),
        (264, "liuwan/jianshanzi/jianshanzi"),
    ] {
        let row = record(&input, page, key);
        assert!(row.observation.coordinate.is_some());
        assert!(request.excluded.iter().any(|a| a.record_id == row.id));
        assert!(!request.units.iter().flat_map(|u|&u.anchors).any(|a|matches!(a,crate::derivation::AnchorReference::Record{record_id} if *record_id==row.id)));
    }
    for (page, key) in [
        (248, "shatian/muwo/banpo"),
        (264, "liuwan/jianshanzi/damawo"),
    ] {
        let row = record(&input, page, key);
        assert!(row.checked);
        assert!(request.units.iter().flat_map(|u|&u.anchors).any(|a|matches!(a,crate::derivation::AnchorReference::Record{record_id} if *record_id==row.id)));
    }
}
