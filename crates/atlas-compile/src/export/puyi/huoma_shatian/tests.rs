use super::*;

#[test]
fn reviewed_pages_round_trip_without_changing_published_commune_and_brigade_ids() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [(245, 8), (246, 31), (247, 8), (248, 36), (249, 22)] {
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
            "page-246/huoma-commune",
            "cb4b24bf-85bc-5772-b923-43b1800bd03f",
        ),
        (
            "page-248/shatian-commune",
            "0590e311-4380-5df8-87c8-725e3ec1d3ee",
        ),
        (
            "p248/shatian/muwo-brigade",
            "94dd1803-f2ad-521d-bb56-b84105c2f081",
        ),
        (
            "p248/shatian/muwo/banpo",
            "34bcb3a6-c41d-5a86-845f-cce88b72cadd",
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
    let unresolved = record(&input, 246, "huoma/huoma/xiutouzhai");
    let published = crate::export::tests::published_record(&output, unresolved);
    assert!(published["observation"]["coordinate"].is_null());
    assert_eq!(
        published["observation"]["unresolved_coordinate"]["latitude_text"],
        "27°39,"
    );
    assert_eq!(published["observation"]["population"], 134);
    assert_eq!(
        record(&input, 248, "shatian/shatian/tengjiazhai")
            .observation
            .name,
        "滕家寨"
    );
}

#[test]
fn networks_preserve_all_resident_entries_seats_and_unlocated_natural_features() {
    let input = input();
    for (commune, nodes, residents, facilities, unlocated) in
        [("huoma", 30, 21, 2, 2), ("shatian", 60, 48, 1, 3)]
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
            (EntityKind::Facility, facilities),
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
        let seats: BTreeSet<_> = network
            .links
            .iter()
            .filter(|l| l.relation == RelationKind::Seat)
            .map(|l| (l.from, l.to))
            .collect();
        assert_eq!(seats.len(), 4);
        let brigade_seats = if commune == "huoma" {
            [
                ("huoma", "zhongjiazhai"),
                ("longjiazhai", "longjiazhai"),
                ("daojiao", "pingzishang"),
            ]
        } else {
            [
                ("shatian", "qiaobian"),
                ("muwo", "banpo"),
                ("baigai", "yakoushang"),
            ]
        };
        for (brigade, seat) in brigade_seats {
            assert!(seats.contains(&(
                id(&format!("{commune}/{brigade}-brigade")),
                id(&format!("{commune}/{brigade}/{seat}"))
            )));
        }
    }
    let group = record(&input, 246, "huoma/huoma/huoma");
    assert!(group.observation.population.is_none());
    assert_eq!(
        group.observation.note.as_deref(),
        Some("片村，包括：陈家寨、王家寨、一把伞等")
    );
    assert_ne!(
        group.entity_id,
        record(&input, 246, "huoma/huoma/huoma-toponym").entity_id
    );
    for (page, key) in [(246, "huoma/school"), (248, "shatian/primary-school")] {
        assert!(record(&input, page, key).observation.population.is_none());
    }
}

#[test]
fn repeated_names_printed_spellings_and_ethnic_labels_retain_distinct_rows() {
    let input = input();
    let west = record(&input, 249, "shatian/baigai/baigai-west");
    let east = record(&input, 249, "shatian/baigai/baigai-east");
    assert_eq!(west.observation.name, "摆盖");
    assert_eq!(east.observation.name, "摆盖");
    assert_eq!(west.observation.population, Some(57));
    assert_eq!(east.observation.population, Some(78));
    assert_eq!(west.observation.parent_id, east.observation.parent_id);
    assert_ne!(west.entity_id, east.entity_id);
    assert_ne!(west.observation.coordinate, east.observation.coordinate);
    for (page, key, spelling) in [
        (246, "huoma/longjiazhai/shangjiayakou", "Shōngjiāyàkǒu"),
        (248, "shatian/shatian/gaogengzi", "Gāngěngzǐ"),
        (248, "shatian/shatian/qingganglin", "Qīngguānglín"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(spelling)
        );
    }
    assert_eq!(
        record(&input, 249, "shatian/muwo/dalubian")
            .observation
            .ethnicities,
        ["汉", "南京"]
    );
    assert_eq!(
        record(&input, 249, "shatian/baigai/zongjiazhai")
            .observation
            .ethnicities,
        ["京"]
    );
    for (key, note) in [
        ("xiaomuwo", "包括下、中沟、龙家寨"),
        ("damuwo", "包括大路边、河沟头、中厂、木窝、半坡"),
    ] {
        let row = record(&input, 249, &format!("shatian/muwo/{key}"));
        assert!(row.observation.population.is_none());
        assert_eq!(row.observation.note.as_deref(), Some(note));
    }
}

#[test]
fn overview_population_disagreement_and_abbreviated_establishment_years_survive() {
    let input = input();
    let overview = record(&input, 245, "huoma-commune/population");
    let table = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/page-246/huoma-commune")
        .unwrap();
    assert_eq!(overview.entity_id, table.entity_id);
    assert_eq!(overview.observation.population, Some(2794));
    assert_eq!(overview.observation.households, Some(788));
    assert_eq!(table.observation.population, Some(3794));
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (page, commune) in [(245, "huoma"), (247, "shatian")] {
        let origin = record(&input, page, &format!("{commune}-commune/establishment"));
        assert_eq!(origin.source_date.as_ref().unwrap().text, "58年");
        for (year, expected) in [(1957, 0), (1959, 1)] {
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
                    .find(|e| e.entity_id == id(&format!("{commune}-commune")))
                    .map(|e| e.origins.len())
                    .unwrap_or_default(),
                expected
            );
        }
    }
}

#[test]
fn river_reaches_confluences_and_same_named_villages_keep_their_source_scopes() {
    let input = input();
    let village = record(&input, 249, "shatian/baigai/liuerhe");
    let river = record(&input, 249, "shatian/liuer-river");
    assert_eq!(village.observation.name, river.observation.name);
    assert_ne!(village.entity_id, river.entity_id);
    assert_eq!(village.observation.population, Some(121));
    assert!(river.observation.coordinate.is_none());
    assert!(
        river
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约8公里")
    );
    let main = record(&input, 246, "chishui-river");
    assert!(main.observation.parent_id.is_none());
    assert!(main.observation.coordinate.is_none());
    assert!(
        main.observation
            .extent
            .as_ref()
            .unwrap()
            .contains("北面边境")
    );
    assert_eq!(
        main.entity_id,
        record(&input, 245, "chishui-river/border").entity_id
    );
    let tributary = record(&input, 246, "huoma/wangxiang-river");
    assert!(
        tributary
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("发源沙田")
    );
    assert!(
        tributary
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约2.5公里")
    );
    let confluence = record(&input, 247, "shatian/baigai-river/confluence");
    assert_eq!(
        confluence.observation.extent.as_deref(),
        Some("老鹰岩下汇入溜耳河。")
    );
    assert_eq!(
        record(&input, 249, "shatian/houzi-mountain")
            .observation
            .note
            .as_deref(),
        Some("山峰，海拔1571.0米")
    );
}
