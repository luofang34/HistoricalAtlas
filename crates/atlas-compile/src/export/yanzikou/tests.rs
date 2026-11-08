use super::*;
use crate::input::load_blocking;
use atlas_boundary::nodal::{NodalBuilder, NodalQuery, NodalScope, RelationKind};
use atlas_core::calendar::{Calendar, CivilDate};
use atlas_ledger::{
    model::EntityKind,
    timeline::{TimeSelection, Timeline},
};

#[path = "communes/tests.rs"]
mod communes;

#[path = "xianjin_efeng/tests.rs"]
mod xianjin_efeng;

#[path = "taiji/tests.rs"]
mod taiji;

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
fn source_tables_publish_all_fields_and_keep_blank_demographics() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [(209, 14), (210, 36), (211, 35), (212, 24)] {
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
    let blank = record(&input, 211, "yanzikou/goutou");
    assert!(blank.observation.population.is_none());
    assert!(blank.observation.ethnicities.is_empty());
    for (key, population) in [("dapingzi", 81), ("jianzhulin", 28)] {
        let source = record(&input, 212, &format!("yanzikou/{key}"));
        let published = crate::export::tests::published_record(&output, source);
        assert_eq!(published["observation"]["population"], population);
        assert_eq!(
            published["observation"]["ethnicities"],
            serde_json::json!([])
        );
    }
}

#[test]
fn commune_network_uses_explicit_brigade_membership_and_seats() {
    let input = input();
    let network = NodalBuilder::new(&input.catalog, &input.data)
        .unwrap()
        .build(NodalQuery {
            entity_id: id("yanzikou-commune"),
            scope: NodalScope::Source {
                source_id: "bijie-gazetteer".into(),
            },
        })
        .unwrap();
    assert_eq!(network.nodes.len(), 86);
    let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
    assert_eq!(
        input
            .data
            .entities
            .iter()
            .filter(|e| { members.contains(&e.id) && e.kind == EntityKind::Settlement })
            .count(),
        74
    );
    assert!(network.nodes.iter().all(|n| !n.anchors.is_empty()));
    for (unit, seat) in [
        ("yanzikou-commune", "yanzikou"),
        ("yanzikou/yanzikou-brigade", "yanzikou"),
        ("yanzikou/zhonggou-brigade", "zhonggou"),
        ("yanzikou/yuntai-brigade", "aluomi"),
        ("yanzikou/shijie-brigade", "baishijie"),
        ("yanzikou/ganhegou-brigade", "tianba"),
        ("yanzikou/huangnipo-brigade", "huangnipo"),
        ("yanzikou/xiongshi-brigade", "banlishu"),
        ("yanzikou/luofu-brigade", "zhongba"),
        ("yanzikou/jinzhu-brigade", "xinyingpan"),
    ] {
        assert!(network.links.iter().any(|link| {
            link.from == id(unit)
                && link.to == id(&format!("yanzikou/{seat}"))
                && link.relation == RelationKind::Seat
        }));
    }
    let parents: BTreeSet<_> = network
        .links
        .iter()
        .filter(|link| {
            link.to == id("yanzikou/matanghe") && link.relation == RelationKind::Subordinate
        })
        .map(|link| link.from)
        .collect();
    assert_eq!(parents, BTreeSet::from([id("yanzikou/luofu-brigade")]));
    assert!(!members.contains(&id("yanzikou/gaojiashan-mountain")));
    let district = record(&input, 210, "yanzikou-district");
    assert!(district.observation.coordinate.is_none());
    assert_eq!(
        record(&input, 209, "yanzikou-district/seat")
            .observation
            .seat_id,
        Some(id("yanzikou/yanzikou"))
    );
}

#[test]
fn coincident_and_same_named_places_preserve_distinct_source_identities() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, key, name, kind, latitude) in [
        (
            210,
            "shijie-shangzhai",
            "上寨",
            EntityKind::Settlement,
            "27°33′",
        ),
        (
            211,
            "xiongshi-shangzhai",
            "上寨",
            EntityKind::Settlement,
            "27°36′",
        ),
        (
            210,
            "gaojiashan",
            "高家山",
            EntityKind::Settlement,
            "27°36′",
        ),
        (
            212,
            "gaojiashan-mountain",
            "高家山",
            EntityKind::Mountain,
            "27°37′",
        ),
        (211, "ganjiadi", "甘家地", EntityKind::Toponym, "27°35′"),
        (212, "yanbei", "岩背", EntityKind::Toponym, "27°33′"),
    ] {
        let key = format!("yanzikou/{key}");
        let source = record(&input, page, &key);
        let published = crate::export::tests::published_record(&output, source);
        assert_eq!(published["observation"]["name"], name);
        assert_eq!(
            published["observation"]["coordinate"]["latitude_text"],
            latitude
        );
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|e| e.id == id(&key))
                .unwrap()
                .kind,
            kind
        );
    }
    for (page, key, pinyin) in [
        (210, "ganhegou-brigade", "Gānghégōu Dàduì"),
        (211, "pingjie", "Pīnjiē"),
        (211, "dawotang", "Dàuōtáng"),
        (212, "fankaimen", "Fǎnkamén"),
        (212, "gaojiashan-mountain", "Gānjiā Shān"),
    ] {
        let source = record(&input, page, &format!("yanzikou/{key}"));
        let published = crate::export::tests::published_record(&output, source);
        assert_eq!(published["observation"]["pinyin"], pinyin);
    }
    let subtotal: u64 = input
        .data
        .records
        .iter()
        .filter(|r| r.observation.parent_id == Some(id("yanzikou-commune")))
        .filter_map(|r| r.observation.population)
        .sum();
    assert_eq!(subtotal, 13687);
    let commune = record(&input, 210, "yanzikou-commune");
    let published = crate::export::tests::published_record(&output, commune);
    assert_eq!(published["observation"]["population"], 14221);
}

#[test]
fn district_names_and_commune_establishment_follow_separate_chronologies() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, key, expected) in [
        (1949, "yanzikou-district", None),
        (1951, "yanzikou-district", Some("七区")),
        (1957, "yanzikou-district", Some("燕子口区")),
        (1957, "yanzikou-commune", None),
        (1959, "yanzikou-commune", Some("燕子口公社")),
    ] {
        let at = CivilDate {
            year,
            month: 8,
            day: 15,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, TimeSelection::Nominal).unwrap();
        let names: BTreeSet<_> = slice
            .entities
            .iter()
            .filter(|e| e.entity_id == id(key))
            .flat_map(|e| &e.properties)
            .flat_map(|p| &p.candidates)
            .filter_map(|c| match &c.assertion.value {
                TemporalValue::Name(value) => Some(value.text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names, expected.into_iter().collect());
    }
    for page in [210, 211, 212] {
        assert!(
            input
                .data
                .records
                .iter()
                .filter(|r| { r.source_id == "bijie-gazetteer" && r.scan_page == Some(page + 35) })
                .all(|r| r.source_date.is_none())
        );
    }
}
