use super::*;

#[test]
fn complete_tables_and_overviews_publish_without_replacing_anchor_identities() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (page, count) in [
        (281, 12),
        (282, 33),
        (283, 35),
        (284, 22),
        (285, 13),
        (286, 37),
        (287, 12),
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
            "page-282/sili-commune",
            "1e94e53f-871c-51c0-8af3-87259f77b113",
        ),
        (
            "page-286/zongji-commune",
            "2f45330f-26c4-5293-8566-6fe9338a1c8c",
        ),
        (
            "p284/sili/lianfeng-brigade",
            "79e96768-805c-54a7-952c-58e0e506e755",
        ),
        (
            "p284/sili/lianfeng/damiaozhai",
            "e2cb5834-87b8-53a5-901b-9eb9565bcffc",
        ),
        (
            "p284/sili/lianfeng/pingzishang",
            "95f72774-eb0c-58c6-bdd0-d0f353278ac8",
        ),
        (
            "p284/sili/yanwan-brigade",
            "d3fe8eae-f43d-5eab-86ed-4196df7a4b33",
        ),
        (
            "p284/sili/yanwan/zengjiayakou",
            "0296bc5d-a658-58b4-b86e-6448fb609dbb",
        ),
        (
            "p284/sili/yanwan/shiyakou",
            "f5a05ec2-c3cb-5e16-8f35-d768fbc2371d",
        ),
    ] {
        assert_eq!(record(&input, key).id.0.to_string(), expected);
    }
}

#[test]
fn networks_include_brigades_seats_and_unlocated_features() {
    let input = input();
    for (commune, nodes, settlements, brigades, seats) in
        [("sili", 92, 75, 7, 8), ("zongji", 52, 39, 6, 7)]
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
            3
        );
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
}

#[test]
fn continued_rows_and_commune_seats_keep_their_source_membership() {
    let input = input();
    for (key, parent) in [
        ("p283/sili/fujiagou/qingganglin", "sili/fujiagou-brigade"),
        ("p283/sili/fujiagou/nanmushu", "sili/fujiagou-brigade"),
        ("p283/sili/fujiagou/datianyakou", "sili/fujiagou-brigade"),
        ("p284/sili/xingfu/wangjiadi", "sili/xingfu-brigade"),
        ("p284/sili/xingfu/dagou", "sili/xingfu-brigade"),
        ("p284/sili/xingfu/wangerguanshan", "sili/xingfu-brigade"),
        ("p287/zongji/primary-school", "zongji-commune"),
        ("p287/zongji/road-station", "zongji-commune"),
    ] {
        assert_eq!(record(&input, key).observation.parent_id, Some(id(parent)));
    }
    assert_eq!(
        record(&input, "page-286/zongji-commune")
            .observation
            .seat_id,
        Some(id("zongji/nanshan/wanyaoshu"))
    );
    assert_eq!(
        record(&input, "p286/zongji/nanshan-brigade")
            .observation
            .seat_id,
        Some(id("zongji/nanshan/nanshanpu"))
    );
    let brigade = record(&input, "p283/sili/niuchang-brigade");
    let seat = record(&input, "p283/sili/niuchang/cuojitian");
    assert_eq!(brigade.observation.seat_id, Some(seat.entity_id));
    assert_eq!(
        brigade
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .longitude_text,
        "105°27′"
    );
    assert_eq!(
        seat.observation.coordinate.as_ref().unwrap().longitude_text,
        "105°29′"
    );
}

#[test]
fn same_named_places_retain_their_types_membership_and_individual_coordinates() {
    let input = input();
    for (a, b) in [
        (
            "p283/sili/fujiagou/qingganglin",
            "p283/sili/niuchang/qingganglin",
        ),
        (
            "p284/sili/lianfeng/pingzishang",
            "p284/sili/yanwan/pingzishang",
        ),
        (
            "p286/zongji/zongji/heduimen",
            "p287/zongji/nanfeng/heduimen",
        ),
    ] {
        let (a, b) = (record(&input, a), record(&input, b));
        assert_eq!(a.observation.name, b.observation.name);
        assert_ne!(a.entity_id, b.entity_id);
        assert_ne!(a.observation.parent_id, b.observation.parent_id);
        assert_ne!(a.observation.coordinate, b.observation.coordinate);
    }
    for (a, b, other_kind) in [
        (
            "p282/sili/fujiagou/lujiagou",
            "p283/sili/xingfu/lujiagou",
            EntityKind::Toponym,
        ),
        (
            "p284/sili/lianfeng/wangguanhe",
            "p284/sili/wangguan-river",
            EntityKind::River,
        ),
        (
            "p284/sili/xingfu/wangerguanshan",
            "p284/sili/wangerguan-mountain",
            EntityKind::Mountain,
        ),
        (
            "p286/zongji/xinlong/tuershan",
            "p285/zongji/tuer-mountain/elevation",
            EntityKind::Mountain,
        ),
    ] {
        let (a, b) = (record(&input, a), record(&input, b));
        assert_eq!(a.observation.name, b.observation.name);
        assert_ne!(a.entity_id, b.entity_id);
        for (entity, kind) in [
            (a.entity_id, EntityKind::Settlement),
            (b.entity_id, other_kind),
        ] {
            assert_eq!(
                input
                    .data
                    .entities
                    .iter()
                    .find(|e| e.id == entity)
                    .unwrap()
                    .kind,
                kind
            );
        }
    }
    assert_eq!(
        record(&input, "p282/sili/fujiagou/lujiagou")
            .observation
            .coordinate,
        record(&input, "p283/sili/xingfu/lujiagou")
            .observation
            .coordinate
    );
}

#[test]
fn printed_spellings_and_blank_cells_survive_publication() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (key, pinyin) in [
        ("p282/sili/yulong/xiaoyangou", "Xǎiyàngōu"),
        ("p282/sili/yulong/dianzishang", "Diànzǐshàn"),
        ("p283/sili/xingfu/hebadadi", "Hédàdàdì"),
        ("p284/sili/lianfeng/qinggangzhai", "Qiānggāngzhài"),
        ("p284/sili/yanwan/batang", "Bàtián"),
        ("p286/zongji/zongji-brigade", "Zǒngjī Càduì"),
        ("p286/zongji/zhongxing-brigade", "Zhōngxīn Dàduì"),
        ("p286/zongji/zhongxing/mawo", "MáoWō"),
        ("p287/zongji/nanfeng-brigade", "Nánfēng Dàqiù"),
        ("p287/zongji/nanfeng/hulutang", "Fúlútáng"),
    ] {
        let row = record(&input, key);
        assert_eq!(
            crate::export::tests::published_record(&output, row)["observation"]["pinyin"],
            pinyin
        );
    }
    for key in [
        "p282/sili/sili/longdongbian",
        "p283/sili/xingfu/yanzishi",
        "p284/sili/xingfu/dagou",
        "p286/zongji/xinlong/baludi",
    ] {
        let row = record(&input, key);
        assert!(row.observation.population.is_none());
        assert!(row.observation.ethnicities.is_empty());
        assert!(row.observation.coordinate.is_some());
    }
    let unclear = record(&input, "p282/sili/fujiagou/longhuotian");
    assert!(unclear.observation.pinyin.is_none());
    assert!(
        unclear
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("unclear medial vowel")
    );
    assert_eq!(
        record(&input, "p282/sili/fujiagou-brigade")
            .observation
            .name,
        "傅家沟大队"
    );
    assert_eq!(
        record(&input, "p282/sili/fujiagou/fujiagou")
            .observation
            .name,
        "付家沟"
    );
    assert_eq!(
        record(&input, "p286/zongji/xinlong/tianxingqiao")
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .latitude_text,
        "27°24′"
    );
}

#[test]
fn conflicting_totals_elevations_and_neighbor_names_keep_their_source_pages() {
    let input = input();
    for (commune, table_page, overview_page, table_pop, overview_pop, households) in [
        ("sili", 282, 281, 7774, 8001, 1522),
        ("zongji", 286, 285, 6385, 6583, 1350),
    ] {
        let table = record(&input, &format!("page-{table_page}/{commune}-commune"));
        let overview = record(
            &input,
            &format!("p{overview_page}/{commune}-commune/population"),
        );
        assert_eq!(table.observation.population, Some(table_pop));
        assert_eq!(overview.observation.population, Some(overview_pop));
        assert_eq!(overview.observation.households, Some(households));
        let sum: u64 = input
            .data
            .records
            .iter()
            .filter(|r| {
                r.observation.rank == Some(atlas_core::assertion::AdministrativeRank::Brigade)
                    && r.observation.parent_id == Some(table.entity_id)
            })
            .filter_map(|r| r.observation.population)
            .sum();
        assert_eq!(sum, table_pop);
    }
    let table = record(&input, "p284/sili/lengsha-ridge");
    let overview = record(&input, "p281/sili/lengsha-ridge/elevation");
    assert_eq!(table.entity_id, overview.entity_id);
    assert!(table.observation.note.as_ref().unwrap().contains("1459.2"));
    assert!(overview.verbatim_quote.as_ref().unwrap().contains("1572"));
    assert!(
        record(&input, "p281/sili-commune/extent")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("北纬27°31至27°35′")
    );
    let zongji = record(&input, "p285/zongji-commune/extent");
    let neighbor = &zongji.observation.territory.as_ref().unwrap().neighbors[0];
    assert_eq!(neighbor.name, "大吉");
    assert_eq!(neighbor.entity_id, Some(id("daji-commune")));
    assert!(
        zongji
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("北纬27°25′～28′")
    );
    assert_eq!(
        record(&input, "p286/zongji/xinlong/wagangzhai")
            .observation
            .name,
        "瓦缸寨"
    );
    assert!(
        record(&input, "p285/zongji/coal-deposit/extent")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("瓦岗寨")
    );
}

#[test]
fn commune_school_and_forest_origins_preserve_their_cited_years() {
    use atlas_core::{origin::OriginTime, time::TimePrecision};
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (key, page, year, date) in [
        ("sili-commune", 281, 1958, "58年"),
        ("zongji-commune", 285, 1958, "1958年"),
        ("sili/dahaizi-forest", 281, 1975, "1975年"),
        ("zongji/primary-school", 285, 1969, "1969年"),
    ] {
        let row = record(&input, &format!("p{page}/{key}/establishment"));
        assert_eq!(row.source_date.as_ref().unwrap().text, date);
        let estimate = input
            .data
            .estimates
            .iter()
            .find(|e| {
                e.entity_id == row.entity_id && matches!(e.value, EstimateValue::Origin { .. })
            })
            .unwrap();
        assert_eq!(estimate.basis, [row.id]);
        let EstimateValue::Origin {
            time: OriginTime::Estimated { instant },
        } = &estimate.value
        else {
            panic!("expected year interval")
        };
        assert_eq!(instant.precision, TimePrecision::Year);
        for (at_year, count) in [(year - 1, 0), (year + 1, 1)] {
            let at = CivilDate {
                year: at_year,
                month: 1,
                day: 1,
            }
            .to_epoch(Calendar::Gregorian, 0)
            .unwrap();
            let slice = timeline.at(at, TimeSelection::Certain).unwrap();
            assert_eq!(
                slice
                    .entities
                    .iter()
                    .find(|e| e.entity_id == row.entity_id)
                    .map(|e| e.origins.len())
                    .unwrap_or_default(),
                count
            );
        }
    }
    assert!(
        record(&input, "p281/sili-commune/establishment")
            .source_date
            .as_ref()
            .unwrap()
            .conversion_basis
            .is_some()
    );
    assert!(
        record(&input, "p285/zongji/nanshan/nanshanpu/fortress")
            .source_date
            .is_none()
    );
    assert!(
        record(&input, "p281/sili/xiaobailowa-pumping-station/irrigation")
            .source_date
            .is_none()
    );
}
