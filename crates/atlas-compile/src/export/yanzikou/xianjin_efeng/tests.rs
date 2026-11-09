use super::*;

#[test]
fn reviewed_pages_publish_all_rows_with_source_notation_and_blank_fields() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (221, 11),
        (222, 36),
        (223, 3),
        (225, 13),
        (226, 37),
        (227, 11),
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
    let unresolved = record(&input, 222, "xianjin/yangliuwan");
    let published = crate::export::tests::published_record(&output, unresolved);
    assert!(published["observation"]["coordinate"].is_null());
    assert_eq!(
        published["observation"]["unresolved_coordinate"]["latitude_text"],
        "72°32′"
    );
    assert_eq!(published["observation"]["population"], 8);
    let malformed = record(&input, 227, "efeng/maocao-mountain");
    let published = crate::export::tests::published_record(&output, malformed);
    assert!(published["observation"]["coordinate"].is_null());
    assert_eq!(
        published["observation"]["unresolved_coordinate"]["longitude_text"],
        "105°28°"
    );
    for (page, key) in [(222, "xianjin/changwanzi"), (226, "efeng/shaguotian")] {
        let source = record(&input, page, key);
        assert!(source.observation.population.is_none());
        assert!(source.observation.ethnicities.is_empty());
    }
    assert!(
        record(&input, 227, "efeng/laoyingpo-reservoir")
            .observation
            .coordinate
            .is_none()
    );
}

#[test]
fn brigade_networks_preserve_unlocated_residents_and_explicit_seats() {
    let input = input();
    let builder = NodalBuilder::new(&input.catalog, &input.data).unwrap();
    for (commune, nodes, settlements, unlocated) in [("xianjin", 35, 31, 1), ("efeng", 39, 30, 0)] {
        let network = builder
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
            (EntityKind::Settlement, settlements),
            (EntityKind::Brigade, 3),
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
            4
        );
        if commune == "xianjin" {
            let missing = network
                .nodes
                .iter()
                .find(|n| n.entity_id == id("xianjin/yangliuwan"))
                .unwrap();
            assert!(missing.anchors.is_empty());
            assert!(network.links.iter().any(|l| {
                l.from == id("xianjin/wuliqiao-brigade")
                    && l.to == missing.entity_id
                    && l.relation == RelationKind::Subordinate
            }));
        }
    }
    for (page, unit, seat) in [
        (222, "xianjin-commune", "xianjin/xiaoshao"),
        (222, "xianjin/yixin-brigade", "xianjin/sunjia-pu"),
        (226, "efeng-commune", "efeng/lujiawan"),
        (226, "efeng/efenggou-brigade", "efeng/dengzhanping"),
    ] {
        assert_eq!(
            record(&input, page, unit).observation.seat_id,
            Some(id(seat))
        );
    }
}

#[test]
fn same_names_preserve_distinct_mountains_and_cite_repeated_cave_entries() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    let west = record(&input, 223, "xianjin/lengsha-liangzi-west");
    let east = record(&input, 223, "xianjin/lengsha-liangzi-east");
    assert_eq!(west.observation.name, east.observation.name);
    assert_ne!(west.entity_id, east.entity_id);
    assert_ne!(west.observation.coordinate, east.observation.coordinate);
    for (source, elevation) in [(west, "1522.5"), (east, "1572.7")] {
        let published = crate::export::tests::published_record(&output, source);
        assert!(
            published["observation"]["note"]
                .as_str()
                .unwrap()
                .contains(elevation)
        );
    }
    let name = record(&input, 226, "efeng/niujiaodong");
    let cave = record(&input, 227, "efeng/niujiaodong");
    assert_eq!(name.entity_id, cave.entity_id);
    assert_eq!(name.observation.coordinate, cave.observation.coordinate);
    assert_eq!(
        name.observation.parent_id,
        Some(id("efeng/minfeng-brigade"))
    );
    assert_eq!(cave.observation.note.as_deref(), Some("洞穴，高/深7/100"));
    assert_eq!(
        input
            .data
            .entities
            .iter()
            .find(|e| e.id == cave.entity_id)
            .unwrap()
            .kind,
        EntityKind::Cave
    );
    let first = record(&input, 222, "xianjin/matanghe");
    let second = record(&input, 212, "yanzikou/matanghe");
    assert_eq!(first.observation.coordinate, second.observation.coordinate);
    assert_ne!(first.entity_id, second.entity_id);
    assert_ne!(first.observation.parent_id, second.observation.parent_id);
    assert_eq!(first.observation.population, Some(70));
    assert_eq!(second.observation.population, Some(86));
    assert_ne!(first.entity_id, id("xianjin/matang-river"));
    assert_ne!(second.entity_id, id("xianjin/matang-river"));
}

#[test]
fn waterworks_and_commune_dates_enter_the_timeline_in_their_cited_years() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for year in [1957, 1959, 1973, 1975, 1976, 1978] {
        let at = CivilDate {
            year,
            month: 8,
            day: 15,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, TimeSelection::Certain).unwrap();
        for (key, established) in [
            ("xianjin-commune", 1958),
            ("efeng-commune", 1958),
            ("efeng/bijiashan-reservoir", 1974),
            ("efeng/laoyingpo-reservoir", 1975),
            ("efeng/xujiaping-reservoir", 1977),
        ] {
            if year == established {
                continue;
            }
            let origins = slice
                .entities
                .iter()
                .find(|e| e.entity_id == id(key))
                .map(|e| e.origins.len())
                .unwrap_or_default();
            assert_eq!(origins, usize::from(year > established), "{key} in {year}");
        }
    }
    for (page, key, text) in [
        (221, "xianjin-commune/establishment", "58年"),
        (225, "efeng/laoyingpo-reservoir/construction", "75年"),
    ] {
        let source_date = record(&input, page, key).source_date.as_ref().unwrap();
        assert_eq!(source_date.text, text);
        assert!(source_date.conversion_basis.is_some());
    }
}
