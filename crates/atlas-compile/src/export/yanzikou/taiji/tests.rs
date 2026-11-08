use super::*;

#[test]
fn taiji_pages_publish_reviewed_fields_and_retain_the_commune_record_identity() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [(229, 10), (230, 35), (231, 35), (232, 36), (233, 5)] {
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
    let commune = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/page-230/taiji-commune")
        .unwrap();
    assert_eq!(
        commune.id.0.to_string(),
        "fc7e5de6-af08-5c32-93b0-a553cfba4503"
    );
    assert_eq!(commune.observation.population, Some(9339));
    assert_eq!(
        commune.observation.seat_id,
        Some(id("taiji/taiji/taijichang"))
    );
    let unresolved = record(&input, 230, "taiji/taiji/xujiapo");
    let published = crate::export::tests::published_record(&output, unresolved);
    assert!(published["observation"]["coordinate"].is_null());
    assert_eq!(
        published["observation"]["unresolved_coordinate"]["longitude_text"],
        "105°30,"
    );
    let blank = record(&input, 230, "taiji/taiji/daguangou");
    assert!(blank.observation.population.is_none());
    assert!(blank.observation.ethnicities.is_empty());
    let aggregate = record(&input, 231, "taiji/feilun/shuitianba");
    assert!(aggregate.observation.population.is_none());
    assert_eq!(aggregate.observation.pinyin.as_deref(), Some("Shuǐtiánbàn"));
    assert!(
        aggregate
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("箐田")
    );
}

#[test]
fn taiji_network_keeps_nine_brigades_ninety_resident_entries_and_ten_seats() {
    let input = input();
    let network = NodalBuilder::new(&input.catalog, &input.data)
        .unwrap()
        .build(NodalQuery {
            entity_id: id("taiji-commune"),
            scope: NodalScope::Source {
                source_id: "bijie-gazetteer".into(),
            },
        })
        .unwrap();
    assert_eq!(network.nodes.len(), 111);
    let members: BTreeSet<_> = network.nodes.iter().map(|n| n.entity_id).collect();
    for (kind, count) in [(EntityKind::Brigade, 9), (EntityKind::Settlement, 90)] {
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
            .nodes
            .iter()
            .filter(|n| n.anchors.is_empty())
            .map(|n| n.entity_id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([id("taiji/taiji/xujiapo"), id("taiji/taiji-river")])
    );
    let seats: BTreeSet<_> = network
        .links
        .iter()
        .filter(|l| l.relation == RelationKind::Seat)
        .map(|l| (l.from, l.to))
        .collect();
    assert_eq!(seats.len(), 10);
    for (unit, seat) in [
        ("taiji", "lujiawan"),
        ("guanhe", "guanzhuang"),
        ("yumin", "shiguancai"),
        ("qingzhu", "dacha"),
        ("qiaotou", "qiaotou"),
        ("feilun", "lijiawan"),
        ("xiongfeng", "hejiazhai"),
        ("guangou", "jinzhulin"),
        ("yangguang", "chenghuangmiao"),
    ] {
        assert!(seats.contains(&(
            id(&format!("taiji/{unit}-brigade")),
            id(&format!("taiji/{unit}/{seat}"))
        )));
    }
}

#[test]
fn taiji_same_names_keep_their_brigade_membership_and_distinct_locations() {
    let input = input();
    for (name, pages_and_keys) in [
        (
            "垭口上",
            vec![
                (230, "guanhe/yakoushang"),
                (231, "qiaotou/yakoushang"),
                (232, "xiongfeng/yakoushang"),
            ],
        ),
        (
            "凉水井",
            vec![
                (232, "xiongfeng/liangshuijing"),
                (232, "guangou/liangshuijing"),
            ],
        ),
        (
            "小坝沟",
            vec![(230, "guanhe/xiaobagou"), (232, "yangguang/xiaobagou")],
        ),
    ] {
        let mut entities = BTreeSet::new();
        let mut parents = BTreeSet::new();
        let mut points = BTreeSet::new();
        for (page, key) in &pages_and_keys {
            let row = record(&input, *page, &format!("taiji/{key}"));
            assert_eq!(row.observation.name, name);
            entities.insert(row.entity_id);
            parents.insert(row.observation.parent_id.unwrap());
            let coordinate = row.observation.coordinate.as_ref().unwrap();
            points.insert((&coordinate.longitude_text, &coordinate.latitude_text));
        }
        assert_eq!(entities.len(), pages_and_keys.len());
        assert_eq!(parents.len(), pages_and_keys.len());
        assert_eq!(points.len(), pages_and_keys.len());
    }
    let village = record(&input, 230, "taiji/taiji/daguangou");
    let locality = record(&input, 232, "taiji/guangou/daguangou");
    assert_eq!(
        village.observation.coordinate,
        locality.observation.coordinate
    );
    assert_ne!(village.entity_id, locality.entity_id);
    assert_ne!(
        village.observation.parent_id,
        locality.observation.parent_id
    );
}

#[test]
fn taiji_elevation_disagreement_and_river_route_remain_attached_to_their_pages() {
    let input = input();
    let overview = record(&input, 229, "taiji/lengsha-liangzi/elevation");
    let table = record(&input, 233, "taiji/lengsha-liangzi");
    assert_eq!(overview.entity_id, table.entity_id);
    assert!(
        overview
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("1553.6")
    );
    assert!(table.observation.note.as_ref().unwrap().contains("1529.2"));
    let river = record(&input, 233, "taiji/taiji-river");
    assert!(river.observation.coordinate.is_none());
    assert!(
        river
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("先进公社五里桥")
    );
    assert_eq!(
        river.entity_id,
        record(&input, 229, "taiji/taiji-river/route").entity_id
    );
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, expected) in [(1957, 0), (1959, 1)] {
        let at = CivilDate {
            year,
            month: 8,
            day: 15,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, TimeSelection::Certain).unwrap();
        let origin_count = slice
            .entities
            .iter()
            .find(|e| e.entity_id == id("taiji-commune"))
            .map(|e| e.origins.len())
            .unwrap_or_default();
        assert_eq!(origin_count, expected);
    }
}
