use super::*;
use atlas_boundary::nodal::{NodalBuilder, NodalQuery, NodalScope, RelationKind};

#[test]
fn commune_networks_draw_source_seats_and_subordinate_settlements() {
    let input = input();
    let builder = NodalBuilder::new(&input.catalog, &input.data).unwrap();
    for (commune, seat, settlements, nodes) in [
        ("jinyinshan", "bainiba", 33, 46),
        ("xinai", "shuiying", 24, 30),
        ("mulang", "wayaoping", 18, 25),
    ] {
        let root = id(&format!("{commune}-commune"));
        let network = builder
            .build(NodalQuery {
                entity_id: root,
                scope: NodalScope::Source {
                    source_id: "bijie-gazetteer".into(),
                },
            })
            .unwrap();
        assert_eq!(network.nodes.len(), nodes);
        let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .filter(|e| { members.contains(&e.id) && e.kind == EntityKind::Settlement })
                .count(),
            settlements
        );
        assert!(network.links.iter().any(|link| {
            link.from == root
                && link.to == id(&format!("{commune}/{seat}"))
                && link.relation == RelationKind::Seat
        }));
        assert!(network.nodes.iter().all(|n| !n.anchors.is_empty()));
        assert!(!members.contains(&id("jinyinshan/zhiqing-farm")));
    }
    let farm = record(&input, 199, "jinyinshan/zhiqing-farm/establishment");
    assert!(farm.observation.coordinate.is_none());
    assert!(farm.observation.parent_id.is_none());
}

#[test]
fn same_named_places_remain_independently_queryable() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, key, population, kind) in [
        (
            201,
            "jinyinshan/banchanggou-south",
            Some(100),
            EntityKind::Settlement,
        ),
        (
            201,
            "jinyinshan/banchanggou-north",
            Some(28),
            EntityKind::Settlement,
        ),
        (
            201,
            "jinyinshan/banchanggou-area",
            None,
            EntityKind::Toponym,
        ),
        (206, "mulang/caizigou", Some(22), EntityKind::Settlement),
        (206, "mulang/caizigou-toponym", None, EntityKind::Toponym),
        (
            206,
            "mulang/luohanshan-north",
            Some(118),
            EntityKind::Settlement,
        ),
        (206, "mulang/luohanshan-south", None, EntityKind::Settlement),
    ] {
        let source = record(&input, page, key);
        assert_eq!(source.entity_id, id(key));
        let published = crate::export::tests::published_record(&output, source);
        assert_eq!(
            published["observation"]["population"],
            serde_json::json!(population)
        );
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|e| e.id == source.entity_id)
                .unwrap()
                .kind,
            kind
        );
    }
    let area = record(&input, 201, "jinyinshan/banchanggou-area");
    assert_eq!(
        area.observation.parent_id,
        Some(id("jinyinshan/banchang-brigade"))
    );
    assert_eq!(
        area.observation.note.as_deref(),
        Some("片村，包括板厂沟、石梯子、垮山、大干沟")
    );
}

#[test]
fn source_coordinate_anomalies_and_rare_names_survive_publication() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, key, longitude, latitude) in [
        (200, "jinyinshan/commune-bowl-factory", "105°23′", "27°23′"),
        (204, "xinai/huachangchang", "107°21′", "27°25′"),
        (204, "xinai/hujia-yakou", "105°21′", "20°23′"),
    ] {
        let source = record(&input, page, key);
        let published = crate::export::tests::published_record(&output, source);
        let coordinate = &published["observation"]["coordinate"];
        assert_eq!(coordinate["longitude_text"], longitude);
        assert_eq!(coordinate["latitude_text"], latitude);
        assert_eq!(coordinate["resolution_arcminutes"], 1.0);
        assert_eq!(published["scan_page"], page + 35);
    }
    let source = record(&input, 204, "xinai/roulangshu");
    let published = crate::export::tests::published_record(&output, source);
    assert_eq!(published["observation"]["name"], "𱣑榔树");
    assert_eq!(published["observation"]["pinyin"], "Ròulángshù");
    assert_eq!(
        record(&input, 204, "xinai/zhongzhai-brigade")
            .observation
            .seat_id,
        Some(source.entity_id)
    );
    assert_eq!(
        record(&input, 199, "jinyinshan-commune/population")
            .observation
            .population,
        Some(4933)
    );
    assert_eq!(
        record(&input, 200, "jinyinshan-commune")
            .observation
            .population,
        Some(4983)
    );
}

#[test]
fn commune_names_follow_establishment_without_redating_source_tables() {
    let input = input();
    for (page, commune, name, date) in [
        (199, "jinyinshan", "金银山公社", "58年"),
        (203, "xinai", "新艾公社", "1958年"),
        (205, "mulang", "牧郎公社", "1958年"),
    ] {
        let key = format!("{commune}-commune");
        assert!(names(&input, &key, 1957, TimeSelection::Nominal).is_empty());
        assert_eq!(
            names(&input, &key, 1959, TimeSelection::Nominal),
            BTreeSet::from([name.to_owned()])
        );
        assert_eq!(
            record(&input, page, &format!("{key}/establishment"))
                .source_date
                .as_ref()
                .unwrap()
                .text,
            date
        );
    }
    for (page, commune) in [(200, "jinyinshan"), (204, "xinai"), (206, "mulang")] {
        assert!(
            record(&input, page, &format!("{commune}-commune"))
                .source_date
                .is_none()
        );
    }
}
