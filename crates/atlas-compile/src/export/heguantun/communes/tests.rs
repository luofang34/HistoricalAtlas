use super::*;
use atlas_ledger::model::EntityKind;

#[test]
fn commune_and_brigade_seats_follow_each_explicit_annotation() {
    let input = input();
    for (page, key, seat) in [
        (138, "daba-commune", "daba/baizhichang"),
        (138, "daba/daba-brigade", "daba/baizhichang"),
        (140, "shanzhai-commune", "shanzhai/shanzhai"),
        (140, "shanzhai/shanzhai-brigade", "shanzhai/yezhutang"),
        (143, "houqing-commune", "houqing/shishan"),
        (143, "houqing/houqing-brigade", "houqing/shuiying"),
        (144, "houqing/liujia-brigade", "houqing/liujiazhai"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.seat_id,
            Some(id(seat))
        );
    }
    let paper = &record(&input, 138, "daba/baizhichang").observation;
    assert!(paper.population.is_none());
    assert!(paper.ethnicities.is_empty());
    assert!(paper.note.as_ref().unwrap().contains("人口例入大坝"));
    assert_eq!(
        record(&input, 138, "daba/daba").observation.population,
        Some(339)
    );
    assert_eq!(
        record(&input, 141, "shanzhai/xiongjiazhai")
            .observation
            .parent_id,
        Some(id("shanzhai/goujiao-brigade"))
    );
}

#[test]
fn coincident_names_retain_distinct_places_and_geographical_kinds() {
    let input = input();
    for (page_a, key_a, page_b, key_b, kind_b) in [
        (
            138,
            "daba/dayandong",
            138,
            "daba/dayan-cave",
            EntityKind::Cave,
        ),
        (
            140,
            "shanzhai/beishuiyandong",
            141,
            "shanzhai/beishuiyan-cave",
            EntityKind::Cave,
        ),
        (
            140,
            "shanzhai/shanzhai",
            140,
            "shanzhai/shanzhai-place",
            EntityKind::Toponym,
        ),
        (
            143,
            "houqing/taoshuwa",
            143,
            "houqing/taoshuwa-place",
            EntityKind::Toponym,
        ),
        (
            144,
            "houqing/hechong",
            144,
            "houqing/hechong-place",
            EntityKind::Toponym,
        ),
        (
            143,
            "houqing/xiaogaoshan",
            145,
            "houqing/xiaogaoshan-mountain",
            EntityKind::Mountain,
        ),
        (
            144,
            "houqing/chahe",
            145,
            "houqing/cha-river",
            EntityKind::River,
        ),
    ] {
        let a = record(&input, page_a, key_a);
        let b = record(&input, page_b, key_b);
        assert_eq!(a.observation.name, b.observation.name);
        assert_ne!(a.entity_id, b.entity_id);
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|e| e.id == b.entity_id)
                .unwrap()
                .kind,
            kind_b
        );
    }
}

#[test]
fn same_named_settlements_in_different_brigades_keep_separate_identities() {
    let input = input();
    for (a, b) in [
        ("houqing/houqing-hujiazhai", "houqing/baiguo-hujiazhai"),
        ("houqing/baiguo-shuangshan", "houqing/liujia-shuangshan"),
    ] {
        let a = input
            .data
            .records
            .iter()
            .find(|r| r.entity_id == id(a))
            .unwrap();
        let b = input
            .data
            .records
            .iter()
            .find(|r| r.entity_id == id(b))
            .unwrap();
        assert_eq!(a.observation.name, b.observation.name);
        assert_ne!(a.entity_id, b.entity_id);
        assert_ne!(a.observation.parent_id, b.observation.parent_id);
    }
}

#[test]
fn overview_and_table_population_disagreement_remains_visible() {
    let input = input();
    let overview = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/p142/houqing-commune/population")
        .unwrap();
    let table = record(&input, 143, "houqing-commune");
    assert_eq!(overview.entity_id, table.entity_id);
    assert_eq!(overview.observation.population, Some(14177));
    assert_eq!(overview.observation.households, Some(2591));
    assert_eq!(table.observation.population, Some(4177));
    assert!(table.observation.households.is_none());
    for key in ["tangguan-brigade", "zhongzhai", "shuitang", "tangguanchong"] {
        assert!(
            record(&input, 144, &format!("houqing/{key}"))
                .observation
                .ethnicities
                .is_empty()
        );
    }
    assert!(
        record(&input, 143, "houqing/baiguo-hujiazhai")
            .observation
            .population
            .is_none()
    );
}

#[test]
fn repeated_geographical_features_share_identity_across_overviews_and_tables() {
    let input = input();
    for (key, pages) in [
        ("heguantun/luohuntai", vec![125, 137, 138]),
        ("heguantun-district/houqing-river", vec![126, 142, 145]),
        ("heguantun-district/limin-reservoir", vec![126, 142, 145]),
        ("daba/shibanjing-sulfur-factory", vec![126, 137]),
    ] {
        for page in pages {
            assert_eq!(record(&input, page, key).entity_id, id(key));
        }
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .filter(|e| e.id == id(key))
                .count(),
            1
        );
    }
    for key in [
        "heguantun-district/houqing-river",
        "houqing/baiguo-river",
        "houqing/cha-river",
    ] {
        let river = &record(&input, 145, key).observation;
        assert!(river.coordinate.is_none());
        assert!(river.extent.as_ref().unwrap().starts_with("发源于"));
    }
    let reservoir = &record(&input, 145, "heguantun-district/limin-reservoir").observation;
    assert!(reservoir.note.as_ref().unwrap().contains("100万立方米"));
    assert_ne!(
        id("houqing/yangjiaba"),
        id("heguantun-district/limin-reservoir")
    );
    assert!(
        record(&input, 144, "houqing/yangjiaba")
            .source_date
            .is_none()
    );
}

#[test]
fn printed_spelling_coordinates_and_cave_dimensions_are_not_normalized() {
    let input = input();
    for (page, key, pinyin) in [
        (138, "daba/zhongzhai", "Zhōngshài"),
        (140, "shanzhai/shanzhai-brigade", "Shāshài Dàduì"),
        (140, "shanzhai/goujiao-brigade", "Gōgjiǎo Dàduì"),
        (143, "houqing/houqing-hujiazhai", "Fújiāzhài"),
        (144, "houqing/limin-brigade", "Límíng Dàduì"),
        (145, "houqing/xiaogaoshan-mountain", "Xiǎgāoshōn"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    let longitude = record(&input, 140, "shanzhai/wanzitou")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(longitude.longitude_text, "105°17′·");
    assert!((longitude.point.longitude - (105.0 + 17.0 / 60.0)).abs() < 1e-12);
    let outlying = record(&input, 143, "houqing/lizishu")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(
        (
            outlying.longitude_text.as_str(),
            outlying.latitude_text.as_str()
        ),
        ("105°09′", "27°17′")
    );
    assert_eq!(
        record(&input, 141, "shanzhai/beishuiyan-cave")
            .observation
            .note
            .as_deref(),
        Some("洞穴1/3")
    );
}

#[test]
fn commune_origins_and_names_start_with_the_stated_establishment_year() {
    let input = input();
    for (key, name) in [
        ("daba-commune", "大坝公社"),
        ("shanzhai-commune", "杉寨人民公社"),
        ("houqing-commune", "后箐人民公社"),
    ] {
        assert!(names(&input, key, 1957, 7).is_empty());
        assert_eq!(names(&input, key, 1959, 7), BTreeSet::from([name.into()]));
        let origins: Vec<_> = input
            .data
            .estimates
            .iter()
            .filter(|estimate| {
                estimate.entity_id == id(key)
                    && matches!(estimate.value, EstimateValue::Origin { .. })
            })
            .collect();
        assert_eq!(origins.len(), 1);
        let EstimateValue::Origin {
            time: atlas_core::origin::OriginTime::Estimated { instant },
        } = origins[0].value
        else {
            panic!("Expected a bounded establishment year")
        };
        assert_eq!(instant.precision, atlas_core::time::TimePrecision::Year);
    }
}
