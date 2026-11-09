use super::*;
use atlas_core::assertion::TemporalValue;
use atlas_ledger::model::EstimateValue;

#[test]
fn reviewed_gongzihe_and_gala_pages_round_trip_with_existing_commune_identities() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, count) in [(265, 9), (266, 27), (267, 10), (268, 34), (269, 1)] {
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
    for (page, commune, expected) in [
        (266, "gongzihe", "9dbec58b-d650-5586-be24-dabd9c6223ca"),
        (268, "gala", "f89d17e6-4f5e-52dd-b9d5-6dc8fa78d792"),
    ] {
        let key = format!("bijie-gazetteer/page-{page}/{commune}-commune");
        let row = input.data.records.iter().find(|r| r.key == key).unwrap();
        assert_eq!(row.id.0.to_string(), expected);
        assert_eq!(row.observation.parent_id, Some(id("puyi-district")));
    }
}

#[test]
fn source_networks_include_brigades_seats_and_unlocated_rivers() {
    let input = input();
    for (commune, nodes, residents, brigades, facilities) in
        [("gongzihe", 27, 20, 3, 1), ("gala", 35, 24, 2, 2)]
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
            1
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
        assert!(!members.contains(&id("gongzihe/houziqiao-power-station")));
    }
}

#[test]
fn population_variants_literal_ethnicities_and_unclear_pinyin_survive_export() {
    let input = input();
    assert_eq!(
        record(&input, 265, "gongzihe-commune/population")
            .observation
            .population,
        Some(2886)
    );
    let table = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/page-266/gongzihe-commune")
        .unwrap();
    assert_eq!(table.observation.population, Some(2885));
    assert_eq!(
        record(&input, 268, "gala/zhongzhai/mengjiagou")
            .observation
            .ethnicities,
        ["京", "汉", "布"]
    );
    let unreadable = record(&input, 268, "gala/zhongzhai/zhuxiaba");
    assert!(unreadable.observation.pinyin.is_none());
    assert!(
        unreadable
            .observation
            .note
            .as_ref()
            .unwrap()
            .contains("Zhūxiàp")
    );
    assert_eq!(
        record(&input, 268, "gala/xingzhai/maoping")
            .observation
            .pinyin
            .as_deref(),
        Some("Mápíng")
    );
    let total: u64 = ["zhongzhai", "xingzhai"]
        .iter()
        .map(|key| {
            record(&input, 268, &format!("gala/{key}-brigade"))
                .observation
                .population
                .unwrap()
        })
        .sum();
    assert_eq!(total, 2507);
    assert_eq!(
        record(&input, 267, "gala-commune/population")
            .observation
            .population,
        Some(2592)
    );
}

#[test]
fn gala_outliers_remain_observations_without_constraining_the_boundary() {
    let input = input();
    let recipe = &input.derivations["bijie-communes-terrain"];
    for (key, latitude) in [("chahe", "27°22′"), ("hanjiagou", "27°30′")] {
        let row = record(&input, 268, &format!("gala/zhongzhai/{key}"));
        assert_eq!(
            row.observation.coordinate.as_ref().unwrap().latitude_text,
            latitude
        );
        assert!(recipe.excluded.iter().any(|e| e.record_id == row.id));
        assert!(!recipe.units.iter().flat_map(|u| &u.anchors).any(|a| matches!(a, crate::derivation::AnchorReference::Record { record_id } if *record_id == row.id)));
    }
    assert_ne!(
        record(&input, 268, "gala/zhongzhai/chahe").entity_id,
        record(&input, 269, "gala/cha-river").entity_id
    );
}

#[test]
fn neighboring_brigade_seats_and_eastern_villages_constrain_the_cited_adjacencies() {
    let input = input();
    let recipe = &input.derivations["bijie-communes-terrain"];
    for (page, key, longitude, latitude, brigade) in [
        (
            279,
            "daji/longling/yuduo",
            "105°33′",
            "27°30′",
            "daji/longling-brigade",
        ),
        (
            279,
            "daji/longling/kuangjiaqing",
            "105°34′",
            "27°30′",
            "daji/longling-brigade",
        ),
        (
            313,
            "hongyan/minzu/tianba",
            "105°35′",
            "27°37′",
            "hongyan/minzu-brigade",
        ),
    ] {
        let row = record(&input, page, key);
        assert!(row.checked);
        assert_eq!(row.scan_page, Some(page + 35));
        let coordinate = row.observation.coordinate.as_ref().unwrap();
        assert_eq!(coordinate.longitude_text, longitude);
        assert_eq!(coordinate.latitude_text, latitude);
        assert_eq!(row.observation.parent_id, Some(id(brigade)));
        assert!(
            recipe
                .units
                .iter()
                .flat_map(|u| &u.anchors)
                .any(|a| matches!(a,
            crate::derivation::AnchorReference::Record { record_id } if *record_id == row.id))
        );
    }
    for (page, key, seat) in [
        (278, "daji/longling-brigade", "daji/longling/yuduo"),
        (312, "hongyan/minzu-brigade", "hongyan/minzu/tianba"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.seat_id,
            Some(id(seat))
        );
    }
}

#[cfg(test)]
fn gala_names(
    input: &LedgerInput,
    year: i64,
    month: u8,
    day: u8,
    selection: TimeSelection,
) -> BTreeSet<String> {
    let at = CivilDate { year, month, day }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap()
        .checked_add(12 * 60 * 60 * 1000)
        .unwrap();
    Timeline::new(&input.catalog, &input.data)
        .unwrap()
        .at(at, selection)
        .unwrap()
        .entities
        .iter()
        .filter(|e| e.entity_id == id("gala-commune"))
        .flat_map(|e| &e.properties)
        .flat_map(|p| &p.candidates)
        .filter_map(|c| match &c.assertion.value {
            TemporalValue::Name(n) => Some(n.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn gala_rename_uses_the_notice_day_and_keeps_its_day_precision() {
    let input = input();
    let rename = record(&input, 267, "gala-commune/renaming");
    assert_eq!(rename.source_date.as_ref().unwrap().text, "1982年8月1日");
    for (year, month, day, name) in [
        (1981, 8, 1, "中寨公社"),
        (1982, 7, 31, "中寨公社"),
        (1982, 8, 2, "戛拉公社"),
    ] {
        assert_eq!(
            gala_names(&input, year, month, day, TimeSelection::Nominal),
            BTreeSet::from([name.into()])
        );
    }
    assert_eq!(
        gala_names(&input, 1982, 8, 1, TimeSelection::Possible),
        BTreeSet::from(["中寨公社".into(), "戛拉公社".into()])
    );
    assert!(gala_names(&input, 1982, 8, 1, TimeSelection::Certain).is_empty());
}

#[test]
fn commune_origins_follow_establishment_years() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for commune in ["bozha", "liuwan", "gongzihe", "gala"] {
        for (year, count) in [(1957, 0), (1959, 1)] {
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
                count
            );
        }
    }
}

#[test]
fn joint_waterworks_and_river_reaches_keep_their_source_scopes() {
    let input = input();
    let joint = record(&input, 265, "gongzihe/houziqiao-power-station/construction");
    assert_eq!(joint.source_date.as_ref().unwrap().text, "1968年");
    assert_eq!(joint.observation.extent.as_deref(), Some("猴子桥"));
    assert!(joint.observation.parent_id.is_none());
    assert!(joint.observation.coordinate.is_none());
    assert!(
        !input
            .data
            .estimates
            .iter()
            .any(|e| e.entity_id == joint.entity_id
                && e.basis.contains(&joint.id)
                && matches!(e.value, EstimateValue::Origin { .. }))
    );
    let waterworks = record(&input, 267, "gala-commune/waterworks");
    assert_eq!(waterworks.source_date.as_ref().unwrap().text, "1980年");
    assert_ne!(
        waterworks.entity_id,
        record(&input, 268, "gala/xiangshuidong-power-station").entity_id
    );
    for (page, key, length) in [
        (266, "gongzihe/wangxiang-river", "11公里"),
        (269, "gala/cha-river", "约七公里"),
    ] {
        let reach = record(&input, page, key);
        assert!(reach.observation.coordinate.is_none());
        assert!(reach.observation.extent.as_ref().unwrap().contains(length));
    }
}
