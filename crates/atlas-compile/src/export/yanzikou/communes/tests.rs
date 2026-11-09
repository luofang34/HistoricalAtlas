use super::*;

#[test]
fn complete_commune_tables_and_overviews_publish_their_reviewed_fields() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (213, 14),
        (214, 37),
        (215, 36),
        (216, 4),
        (217, 16),
        (218, 37),
        (219, 35),
        (220, 16),
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
}

#[test]
fn networks_include_all_residential_localities_and_each_brigade_seat() {
    let input = input();
    let builder = NodalBuilder::new(&input.catalog, &input.data).unwrap();
    for (commune, count, settlements, brigades, seat) in [
        ("mianyuhe", 71, 61, 8, "dayuanzi"),
        ("liangyan", 82, 72, 6, "liangyan"),
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
        assert_eq!(network.nodes.len(), count);
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
                    .filter(|e| { members.contains(&e.id) && e.kind == kind })
                    .count(),
                count
            );
        }
        assert!(network.nodes.iter().all(|n| !n.anchors.is_empty()));
        assert!(network.links.iter().any(|l| {
            l.from == root
                && l.to == id(&format!("{commune}/{seat}"))
                && l.relation == RelationKind::Seat
        }));
        let seated: BTreeSet<_> = network
            .links
            .iter()
            .filter(|l| l.relation == RelationKind::Seat && l.from != root)
            .map(|l| l.from)
            .collect();
        assert_eq!(seated.len(), brigades);
    }
    for (page, brigade, seat) in [
        (214, "mianyuhe/longdong-brigade", "mianyuhe/dache"),
        (214, "mianyuhe/mianyuhe-brigade", "mianyuhe/laofangzi"),
        (219, "liangyan/jinhua-brigade", "liangyan/maoerba"),
    ] {
        assert_eq!(
            record(&input, page, brigade).observation.seat_id,
            Some(id(seat))
        );
    }
}

#[test]
fn coincident_villages_keep_separate_brigades_and_blank_demographics() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    let first = record(&input, 218, "liangyan/sanlitian-yingjiao");
    let second = record(&input, 219, "liangyan/heguang-yingjiao");
    assert_eq!(first.observation.coordinate, second.observation.coordinate);
    assert_ne!(first.entity_id, second.entity_id);
    for (source, brigade, population) in [
        (first, "liangyan/sanlitian-brigade", 63),
        (second, "liangyan/heguang-brigade", 60),
    ] {
        let published = crate::export::tests::published_record(&output, source);
        assert_eq!(
            published["observation"]["parent_id"],
            id(brigade).0.to_string()
        );
        assert_eq!(published["observation"]["population"], population);
    }
    for (key, population) in [
        ("dagoutou", Some(23)),
        ("maoerba", Some(198)),
        ("yanshang", None),
        ("sikuaitian", Some(30)),
        ("wujialing", Some(10)),
    ] {
        let source = record(&input, 219, &format!("liangyan/{key}"));
        let published = crate::export::tests::published_record(&output, source);
        assert_eq!(
            published["observation"]["population"],
            serde_json::json!(population)
        );
        assert_eq!(
            published["observation"]["ethnicities"],
            serde_json::json!([])
        );
    }
    assert_eq!(
        record(&input, 218, "liangyan/maocaoshan")
            .observation
            .population,
        Some(230)
    );
    assert!(
        record(&input, 220, "liangyan/maocaoshan-toponym")
            .observation
            .population
            .is_none()
    );
}

#[test]
fn incomplete_reservoir_coordinate_and_scoped_river_lengths_survive_export() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    let source = record(&input, 220, "liangyan/liziyuan-reservoir");
    let published = crate::export::tests::published_record(&output, source);
    assert!(published["observation"]["coordinate"].is_null());
    let coordinate = &published["observation"]["unresolved_coordinate"];
    assert_eq!(coordinate["longitude_text"], "105°");
    assert_eq!(coordinate["latitude_text"], "27°");
    assert_eq!(coordinate["resolution_arcminutes"], 60.0);
    let whole = record(&input, 213, "duipo/mianyu-river/course");
    let local = record(&input, 215, "duipo/mianyu-river");
    assert_eq!(whole.entity_id, local.entity_id);
    assert_eq!(
        whole.entity_id,
        record(&input, 132, "duipo/mianyu-river").entity_id
    );
    for (source, length) in [(whole, "全长约十九公里"), (local, "境内流长约15公里")] {
        let published = crate::export::tests::published_record(&output, source);
        assert!(published["observation"]["coordinate"].is_null());
        assert!(
            published["observation"]["extent"]
                .as_str()
                .unwrap()
                .contains(length)
        );
    }
}

#[test]
fn merger_and_renaming_do_not_backdate_the_new_commune() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (year, key, expected) in [
        (1965, "mianyuhe-commune", None),
        (1967, "mianyuhe-commune", Some("龙洞人民公社")),
        (1982, "mianyuhe-commune", Some("沔鱼河公社")),
        (1957, "liangyan-commune", None),
        (1959, "liangyan-commune", Some("亮岩人民公社")),
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
    let predecessor = id("mianyuhe/longdong-predecessor-commune");
    assert_ne!(predecessor, id("mianyuhe-commune"));
    assert_ne!(predecessor, id("mianyuhe/longdong-brigade"));
    for (year, established) in [(1976, false), (1979, true)] {
        let at = CivilDate {
            year,
            month: 8,
            day: 15,
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap();
        let slice = timeline.at(at, TimeSelection::Certain).unwrap();
        for key in ["liangyan/liziyuan-reservoir", "liangyan/panlong-reservoir"] {
            let count = slice
                .entities
                .iter()
                .find(|e| e.entity_id == id(key))
                .map(|e| e.origins.len())
                .unwrap_or_default();
            assert_eq!(count, usize::from(established));
        }
    }
    assert_eq!(
        record(&input, 217, "liangyan/liziyuan-reservoir/construction")
            .source_date
            .as_ref()
            .unwrap()
            .text,
        "77年"
    );
}
