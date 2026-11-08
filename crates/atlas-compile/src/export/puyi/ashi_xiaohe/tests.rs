use super::*;

#[test]
fn liuwan_map_symbol_refines_the_boundary_without_replacing_table_coordinates() {
    let input = input();
    let table = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/page-264/liuwan-commune")
        .unwrap();
    let coordinate = table.observation.coordinate.as_ref().unwrap();
    assert_eq!(coordinate.longitude_text, "105°37′");
    assert_eq!(coordinate.latitude_text, "27°36′");
    let map = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/county-map/liuwan-commune")
        .unwrap();
    assert!(map.checked);
    assert_eq!(map.scan_page, Some(658));
    assert_eq!(map.entity_id, table.entity_id);
    let location = input
        .data
        .estimates
        .iter()
        .find(|e| e.key == "bijie-gazetteer/county-map/liuwan-commune/registered-location")
        .unwrap();
    assert!(location.basis.contains(&map.id));
    let atlas_ledger::model::EstimateValue::MapLocation {
        point,
        uncertainty_m,
        datum,
        ..
    } = location.value
    else {
        panic!("map location expected")
    };
    assert!((point.longitude - 105.612918493783).abs() < 1e-10);
    assert!((point.latitude - 27.607604827342065).abs() < 1e-10);
    assert_eq!(uncertainty_m, 3500.);
    assert_eq!(datum, atlas_geo::coordinates::Datum::Unspecified);
    assert_ne!(point, coordinate.point);
    let recipe = &input.derivations["bijie-communes-terrain"];
    assert!(recipe.units.iter().flat_map(|u| &u.anchors).any(|a|
        matches!(a, crate::derivation::AnchorReference::MapLocation { estimate_id } if *estimate_id == location.id)
    ));
    assert!(recipe.excluded.iter().any(|a| a.record_id == table.id));
}

#[test]
fn reviewed_ashi_and_xiaohe_pages_round_trip_with_existing_commune_identities() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [
        (251, 11),
        (252, 35),
        (253, 22),
        (255, 10),
        (256, 32),
        (257, 1),
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
    for (page, commune, population) in [(252, "ashi", 5818), (256, "xiaohe", 2026)] {
        let key = format!("bijie-gazetteer/page-{page}/{commune}-commune");
        let row = input.data.records.iter().find(|r| r.key == key).unwrap();
        assert_eq!(
            row.id,
            atlas_core::identity::FactId::from_key(&key).unwrap()
        );
        assert_eq!(row.observation.population, Some(population));
        assert_eq!(row.observation.parent_id, Some(id("puyi-district")));
    }
    let neighbor = record(&input, 264, "liuwan/jianshanzi/damawo");
    assert!(neighbor.checked);
    assert_eq!(neighbor.observation.population, Some(235));
    assert_eq!(
        neighbor.observation.parent_id,
        Some(id("liuwan/jianshanzi-brigade"))
    );
    assert_eq!(
        neighbor
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .latitude_text,
        "27°37′"
    );
}

#[test]
fn networks_keep_cross_page_brigades_and_separate_residents_from_facilities() {
    let input = input();
    for (commune, nodes, residents, brigades, facilities, unlocated) in
        [("ashi", 59, 48, 4, 1, 2), ("xiaohe", 33, 26, 2, 2, 1)]
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
            (EntityKind::Brigade, brigades),
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
        assert_eq!(
            network
                .links
                .iter()
                .filter(|l| l.relation == RelationKind::Seat)
                .map(|l| (l.from, l.to))
                .collect::<BTreeSet<_>>()
                .len(),
            brigades + 1
        );
    }
    for (page, key, brigade) in [
        (253, "ashi/akaihe/datianjiao", "ashi/akaihe-brigade"),
        (253, "ashi/akaihe/shiyakou", "ashi/akaihe-brigade"),
        (253, "ashi/mawo/sanmawo", "ashi/mawo-brigade"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.parent_id,
            Some(id(brigade))
        );
    }
    let group = record(&input, 256, "xiaohe/yanhe/guantianba");
    assert!(group.observation.population.is_none());
    assert_eq!(group.observation.note.as_deref(), Some("片名，村"));
    assert!(
        record(&input, 256, "xiaohe/yanhe/banpo")
            .observation
            .population
            .is_none()
    );
    assert_eq!(
        record(&input, 256, "xiaohe/yanhe/banpo")
            .observation
            .note
            .as_deref(),
        Some("地名")
    );
}

#[test]
fn same_names_source_spellings_and_demographics_preserve_their_individual_rows() {
    let input = input();
    let south = record(&input, 252, "ashi/mula/shiyakou");
    let north = record(&input, 253, "ashi/akaihe/shiyakou");
    assert_eq!(
        record(&input, 252, "ashi/ashi/lanaotian").observation.name,
        "滥坳田"
    );
    assert_eq!(south.observation.name, north.observation.name);
    assert_ne!(south.entity_id, north.entity_id);
    assert_ne!(south.observation.parent_id, north.observation.parent_id);
    assert_ne!(south.observation.coordinate, north.observation.coordinate);
    assert_eq!(south.observation.population, Some(36));
    assert_eq!(north.observation.population, Some(21));
    let alias = record(&input, 251, "ashi/ashi/ashi/name");
    assert_eq!(alias.observation.former_names[0].name, "发朗");
    assert!(alias.observation.former_names[0].validity.is_none());
    assert_ne!(
        alias.entity_id,
        record(&input, 252, "ashi/akaihe/falang").entity_id
    );
    for (page, key, pinyin) in [
        (252, "ashi/mula/gaoshangtou", "Gānshàngtóu"),
        (256, "xiaohe/yanhe/midujia", "Mīdūjiá"),
        (256, "xiaohe/yanhe/xialaomo", "Xiàlǎmō"),
        (256, "xiaohe/yanhe/yingjiao", "Yīngjiǎo"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    assert_eq!(
        record(&input, 252, "ashi/akaihe/falang")
            .observation
            .ethnicities,
        ["汉", "京"]
    );
    assert_eq!(
        record(&input, 256, "xiaohe/shuanghe/dashibao")
            .observation
            .ethnicities,
        ["汉", "羿"]
    );
    assert_eq!(
        record(&input, 256, "xiaohe/shuanghe-brigade")
            .observation
            .population,
        Some(788)
    );
    assert_eq!(
        record(&input, 256, "xiaohe/yanhe-brigade")
            .observation
            .population,
        Some(1212)
    );
    assert_eq!(
        record(&input, 255, "xiaohe-commune/population")
            .observation
            .population,
        Some(2026)
    );
}

#[test]
fn commune_establishment_years_do_not_date_the_reservoir_or_former_place_name() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (page, commune) in [(251, "ashi"), (255, "xiaohe")] {
        let origin = record(&input, page, &format!("{commune}-commune/establishment"));
        assert_eq!(origin.source_date.as_ref().unwrap().text, "1958年");
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
    let construction = record(&input, 251, "ashi/xiaogelao-reservoir/construction");
    assert_eq!(construction.source_date.as_ref().unwrap().text, "1969年");
    assert!(
        construction
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("兴建")
    );
    assert!(
        !input
            .data
            .estimates
            .iter()
            .any(|e| e.entity_id == construction.entity_id
                && matches!(e.value, atlas_ledger::model::EstimateValue::Origin { .. }))
    );
    assert_eq!(
        construction.entity_id,
        record(&input, 253, "ashi/xiaogelao-reservoir").entity_id
    );
}

#[test]
fn river_reaches_mountains_and_ferry_keep_their_source_scopes() {
    let input = input();
    let river = record(&input, 251, "ashi/akai-river/route");
    assert!(river.observation.coordinate.is_none());
    assert!(
        river
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("河尾巴村伏流")
    );
    assert_ne!(
        river.entity_id,
        record(&input, 252, "ashi/akaihe/akaihe").entity_id
    );
    for (page, key, height) in [
        (253, "ashi/niujianbang-mountain", "山峰，海拔1532.0米"),
        (253, "ashi/laolu-mountain", "山峰，海拔1446.0米"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.note.as_deref(),
            Some(height)
        );
    }
    let reach = record(&input, 256, "xiaohe/xiaohe-river");
    assert!(reach.observation.coordinate.is_none());
    assert!(
        reach
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约4公里")
    );
    assert_eq!(
        reach.entity_id,
        record(&input, 255, "xiaohe/xiaohe-river/route").entity_id
    );
    let village = record(&input, 256, "xiaohe/shuanghe/xiaohe");
    let ferry = record(&input, 257, "xiaohe/xiaohe-ferry");
    assert_ne!(village.entity_id, ferry.entity_id);
    assert_ne!(village.entity_id, reach.entity_id);
    assert_eq!(village.observation.coordinate, ferry.observation.coordinate);
    assert!(ferry.observation.population.is_none());
    let main = record(&input, 255, "chishui-river/xiaohe-border");
    assert!(main.observation.parent_id.is_none());
    assert_eq!(
        main.entity_id,
        record(&input, 246, "chishui-river").entity_id
    );
    assert!(
        main.observation
            .extent
            .as_ref()
            .unwrap()
            .contains("5.5公里")
    );
}
