use super::*;
use atlas_boundary::nodal::{NodalBuilder, NodalQuery, NodalScope, RelationKind};

#[test]
fn commune_networks_keep_colocated_seats_and_every_subordinate_settlement() {
    let input = input();
    let builder = NodalBuilder::new(&input.catalog, &input.data).unwrap();
    for (commune, seat, settlements, nodes) in [
        ("shisanmu", "shisanmu", 30, 34),
        ("yazitang", "dingjiaba", 15, 16),
        ("bazhai", "bazhaiping", 17, 22),
    ] {
        let root = id(&format!("{commune}-commune"));
        let seat = id(&format!("{commune}/{seat}"));
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
                .filter(|e| members.contains(&e.id) && e.kind == EntityKind::Settlement)
                .count(),
            settlements
        );
        assert!(network.links.iter().any(|link| {
            link.from == root && link.to == seat && link.relation == RelationKind::Seat
        }));
        let root_node = network.nodes.iter().find(|n| n.entity_id == root).unwrap();
        let seat_node = network.nodes.iter().find(|n| n.entity_id == seat).unwrap();
        assert_ne!(root_node.entity_id, seat_node.entity_id);
        assert_eq!(
            root_node.anchors[0].coordinate.point,
            seat_node.anchors[0].coordinate.point
        );
        assert!(network.nodes.iter().all(|n| !n.anchors.is_empty()));
    }
}

#[test]
fn source_totals_and_included_populations_remain_separate_in_publication() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, key, population, note) in [
        (195, "yazitang-commune/population", Some(2281), None),
        (196, "yazitang-commune", Some(2229), None),
        (192, "shisanmu/yakoushang", Some(104), Some("人口计入岔河")),
        (192, "shisanmu/xiaopingping", None, Some("人口计入油沙地")),
        (192, "shisanmu/longtanping", None, Some("人口计入岩头上")),
        (192, "shisanmu/sandaogou", None, Some("人口计入红岩头")),
        (192, "shisanmu/shenjiagou", None, Some("人口计入红岩头")),
    ] {
        let source = record(&input, page, key);
        let published = crate::export::tests::published_record(&output, source);
        assert_eq!(
            published["observation"]["population"],
            serde_json::json!(population)
        );
        if let Some(note) = note {
            assert_eq!(published["observation"]["note"], note);
        }
        assert_eq!(published["scan_page"], page + 35);
    }
    for (page, key, river) in [
        (193, "haizijie/haogou-river", "haizijie/haogou-river"),
        (198, "haizijie/qiansuo-river", "haizijie/qiansuo-river"),
    ] {
        let source = record(&input, page, key);
        assert_eq!(source.entity_id, id(river));
        let published = crate::export::tests::published_record(&output, source);
        assert!(published["observation"]["coordinate"].is_null());
        assert!(
            published["observation"]["extent"]
                .as_str()
                .unwrap()
                .contains("发源于本")
        );
    }
    assert_eq!(
        record(&input, 192, "shisanmu/chaqiangyan-brigade")
            .observation
            .ethnicities,
        ["汉", "苗", "京"]
    );
    assert_eq!(
        record(&input, 198, "bazhai/shanshujiao")
            .observation
            .pinyin
            .as_deref(),
        Some("Shāshùjiǎo")
    );
}

#[test]
fn commune_names_start_in_the_reported_establishment_year() {
    let input = input();
    for (page, commune, name, date) in [
        (191, "shisanmu", "十三亩公社", "1958年"),
        (195, "yazitang", "鸭子塘公社", "58年"),
        (197, "bazhai", "八寨公社", "1958年"),
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
    assert!(
        record(&input, 196, "yazitang-commune")
            .source_date
            .is_none()
    );
}
