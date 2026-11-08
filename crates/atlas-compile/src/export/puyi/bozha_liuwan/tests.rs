use super::*;

#[test]
fn reviewed_bozha_and_liuwan_pages_preserve_anchor_ids_and_all_source_fields() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [(259, 10), (260, 34), (261, 5), (263, 9), (264, 30)] {
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
            "page-260/bozha-commune",
            "3f066ddc-d6a3-500b-9171-5faa57c62f03",
        ),
        (
            "page-264/liuwan-commune",
            "41969bab-79bc-512c-9981-0a6527555430",
        ),
        (
            "p264/liuwan/jianshanzi-brigade",
            "09469d90-1c69-5cba-bfb3-17040b29c62b",
        ),
        (
            "p264/liuwan/jianshanzi/jianshanzi",
            "9cf8c7a8-6db1-5056-a147-a3b20edd9d3c",
        ),
        (
            "p264/liuwan/jianshanzi/damawo",
            "1dc395b7-b32b-59d3-9030-466b6ccd9f77",
        ),
    ] {
        let row = input
            .data
            .records
            .iter()
            .find(|r| r.key == format!("bijie-gazetteer/{key}"))
            .unwrap();
        assert_eq!(row.id.0.to_string(), expected);
    }
}

#[test]
fn networks_follow_cross_page_brigades_and_both_liuwan_seat_claims() {
    let input = input();
    for (commune, residents, nodes, seats, unlocated) in
        [("bozha", 31, 41, 4, 3), ("liuwan", 23, 31, 5, 1)]
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
            unlocated
        );
        let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
        for (kind, count) in [
            (EntityKind::Settlement, residents),
            (EntityKind::Brigade, 3),
            (EntityKind::Facility, 2),
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
        let links: BTreeSet<_> = network
            .links
            .iter()
            .filter(|l| l.relation == RelationKind::Seat)
            .map(|l| (l.from, l.to))
            .collect();
        assert_eq!(links.len(), seats);
        if commune == "liuwan" {
            for seat in ["liuwan-locality", "liuwan/dazhai"] {
                assert!(links.contains(&(id("liuwan-commune"), id(&format!("liuwan/{seat}")))));
            }
        }
    }
    for key in ["heduimen", "yantoushang"] {
        assert_eq!(
            record(&input, 261, &format!("bozha/touzhuang/{key}"))
                .observation
                .parent_id,
            Some(id("bozha/touzhuang-brigade"))
        );
    }
}

#[test]
fn grouped_residents_and_same_named_localities_keep_distinct_evidence() {
    let input = input();
    let village = record(&input, 260, "bozha/yakoutian/mufulang");
    let group = record(&input, 260, "bozha/yakoutian/mufulang-group");
    assert_eq!(village.observation.name, group.observation.name);
    assert_ne!(village.entity_id, group.entity_id);
    assert_eq!(village.observation.population, Some(176));
    assert!(group.observation.population.is_none());
    assert_ne!(village.observation.coordinate, group.observation.coordinate);
    assert_eq!(
        group.observation.note.as_deref(),
        Some("包括：下寨、上寨、木夫朗、坝子头")
    );
    assert_eq!(
        record(&input, 260, "bozha/touzhuang/qinggangzhai")
            .observation
            .note
            .as_deref(),
        Some("片村，包括：桥边、营脚、新房子")
    );
    let resident = record(&input, 264, "liuwan/jianshanzi/hebian");
    let locality = record(&input, 264, "liuwan/jianshanzi/hebian-locality");
    assert_ne!(resident.entity_id, locality.entity_id);
    assert_eq!(
        resident.observation.coordinate,
        locality.observation.coordinate
    );
    assert_eq!(resident.observation.population, Some(161));
    assert!(locality.observation.population.is_none());
    assert_eq!(locality.observation.note.as_deref(), Some("地名"));
    let former = record(&input, 263, "liuwan/liuwan-locality/name");
    assert_eq!(former.observation.former_names[0].name, "杨柳湾");
    assert_ne!(
        former.entity_id,
        record(&input, 264, "liuwan/liuwan/yangliuwan").entity_id
    );
}

#[test]
fn source_anomalies_and_river_reaches_remain_independent_of_derived_boundaries() {
    let input = input();
    let brigade = record(&input, 260, "bozha/tuansheng-brigade");
    assert_eq!(
        brigade
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .latitude_text,
        "29°32′"
    );
    assert_eq!(
        brigade.observation.seat_id,
        Some(id("bozha/tuansheng/tianba"))
    );
    assert_eq!(
        record(&input, 260, "bozha/yakoutian/shangzhai")
            .observation
            .pinyin
            .as_deref(),
        Some("shàngzhài")
    );
    assert_eq!(
        record(&input, 264, "liuwan/tiankanzhai/chenjiayakou")
            .observation
            .pinyin
            .as_deref(),
        Some("Chéngjiāyàkǒu")
    );
    let total: u64 = ["liuwan", "jianshanzi", "tiankanzhai"]
        .iter()
        .map(|key| {
            record(&input, 264, &format!("liuwan/{key}-brigade"))
                .observation
                .population
                .unwrap()
        })
        .sum();
    assert_eq!(total, 2900);
    assert_eq!(
        record(&input, 263, "liuwan-commune/population")
            .observation
            .population,
        Some(2922)
    );
    let river = record(&input, 261, "bozha/tianba-river");
    assert!(river.observation.coordinate.is_none());
    assert!(
        river
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("小格老水库")
    );
    assert!(
        river
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约8公里")
    );
    assert_ne!(
        record(&input, 259, "bozha/toudao-river/route").entity_id,
        record(&input, 260, "bozha/tuansheng/toudaohe").entity_id
    );
}
