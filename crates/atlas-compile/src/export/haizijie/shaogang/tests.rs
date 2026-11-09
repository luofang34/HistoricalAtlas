use super::*;
use atlas_core::assertion::AdministrativeRank;

#[test]
fn three_communes_retain_their_brigade_seats_and_explicit_village_rows() {
    let input = input();
    for (page, commune, brigade_count, village_count) in [
        (166, "shaogang", 6, 25),
        (170, "shashao", 5, 14),
        (172, "wangzhang", 4, 20),
    ] {
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| r.scan_page == Some(page + 35))
            .collect();
        let brigades: Vec<_> = rows
            .iter()
            .filter(|r| r.observation.rank == Some(AdministrativeRank::Brigade))
            .collect();
        assert_eq!(brigades.len(), brigade_count);
        assert!(brigades.iter().all(|r| r.observation.parent_id
            == Some(id(&format!("{commune}-commune")))
            && r.observation.seat_id.is_some()));
        let villagers = rows
            .iter()
            .filter(|r| {
                input
                    .data
                    .entities
                    .iter()
                    .find(|e| e.id == r.entity_id)
                    .unwrap()
                    .kind
                    == EntityKind::Settlement
            })
            .count();
        assert_eq!(villagers, village_count);
    }
    for (page, key, seat) in [
        (166, "shaogang-commune", "shaogang/zhangjiazhai"),
        (166, "shaogang/xichonggou-brigade", "shaogang/xichong"),
        (170, "shashao-commune", "shashao/shadi"),
        (170, "shashao/shashao-brigade", "shashao/shadi"),
        (170, "shashao/shaoguan-brigade", "shashao/shaoguanpu"),
        (170, "shashao/dianzi-brigade", "shashao/dianzishang"),
        (170, "shashao/mahuangqing-brigade", "shashao/zhangjiazhai"),
        (172, "wangzhang-commune", "wangzhang/wangzhangjie"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.seat_id,
            Some(id(seat))
        );
    }
    assert!(
        record(&input, 169, "shashao-commune/subdivisions")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("15个自然村")
    );
    assert_eq!(
        input
            .data
            .entities
            .iter()
            .find(|e| e.id == id("shashao/mahuangqing-place"))
            .unwrap()
            .kind,
        EntityKind::Toponym
    );
}

#[test]
fn shared_names_preserve_each_mountain_settlement_and_bridge_identity() {
    let input = input();
    for (page_a, key_a, page_b, key_b) in [
        (166, "shaogang/gongbeiqiao", 166, "shaogang/gongbei-bridge"),
        (170, "shashao/housuodong", 170, "shashao/housuo-cave"),
        (166, "shaogang/baimashan", 167, "shaogang/baima-mountain"),
        (
            167,
            "shaogang/baima-mountain",
            172,
            "wangzhang/baima-mountain",
        ),
    ] {
        let a = record(&input, page_a, key_a);
        let b = record(&input, page_b, key_b);
        assert_eq!(a.observation.name, b.observation.name);
        assert_ne!(a.entity_id, b.entity_id);
    }
    let alias = record(&input, 167, "shaogang/dashan/local-name");
    assert_eq!(alias.entity_id, id("shaogang/dashan"));
    assert_eq!(alias.observation.name, "白马山");
    assert!(alias.observation.former_names.is_empty());
    assert!(alias.observation.coordinate.is_none());
    let rows: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|r| r.scan_page == Some(202) && r.locator.starts_with("Place table, row"))
        .collect();
    assert_eq!(rows.len(), 5);
    assert_eq!(
        record(&input, 166, "shaogang/gongbei-bridge")
            .observation
            .note
            .as_deref(),
        Some("桥 (17.7—3.9)/15")
    );
}

#[test]
fn source_coordinate_outliers_and_unusual_pinyin_survive_compilation() {
    let input = input();
    let coordinate = record(&input, 172, "wangzhang/guojiazui")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(coordinate.longitude_text, "125°25′");
    assert!((coordinate.point.longitude - (125.0 + 25.0 / 60.0)).abs() < 1e-12);
    for key in ["shaogang/baima-mountain", "shaogang/guanmen-mountain"] {
        let coordinate = record(&input, 167, key)
            .observation
            .coordinate
            .as_ref()
            .unwrap();
        assert_eq!(coordinate.latitude_text, "27°29′");
        assert_eq!(coordinate.resolution_arcminutes, 1.0);
    }
    assert_eq!(
        record(&input, 172, "wangzhang/zitong-pavilion")
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .latitude_text,
        "27°27′"
    );
    for (page, key, pinyin) in [
        (167, "shaogang/guankouying", "Cuānkoúyíng"),
        (167, "shaogang/guanmen-mountain", "Cuānmēn Shān"),
        (170, "shashao/linjia-daying", "Línjiā Dàyín"),
        (170, "shashao/dapoying", "Dàpōyín"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
}

#[test]
fn population_rows_keep_blanks_and_source_disagreements() {
    let input = input();
    let meiziqiao = record(&input, 166, "shaogang/meiziqiao");
    assert!(meiziqiao.observation.population.is_none());
    assert!(meiziqiao.observation.ethnicities.is_empty());
    assert_eq!(
        meiziqiao.observation.note.as_deref(),
        Some("村，人口列入梅子沟")
    );
    assert_eq!(
        record(&input, 166, "shaogang/meizigou")
            .observation
            .population,
        Some(627)
    );
    assert_eq!(
        record(&input, 172, "wangzhang/hebian")
            .observation
            .population,
        Some(2052)
    );
    assert_eq!(
        record(&input, 172, "wangzhang/hebian")
            .observation
            .parent_id,
        Some(id("wangzhang/shukua-brigade"))
    );
    assert_eq!(
        record(&input, 172, "wangzhang/shukua-brigade")
            .observation
            .population,
        Some(608)
    );
    assert_eq!(
        record(&input, 172, "wangzhang/baihushan")
            .observation
            .population,
        Some(48)
    );
    assert!(
        record(&input, 171, "wangzhang-commune/population")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("苗族38人")
    );
    assert_eq!(
        record(&input, 170, "shashao-commune")
            .observation
            .ethnicities,
        ["汉", "彝"]
    );
    assert_eq!(
        record(&input, 169, "shashao-commune/population")
            .observation
            .ethnicities,
        ["汉"]
    );
}

#[test]
fn district_facilities_gain_coordinates_without_duplicate_entities() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (key, name, earlier_page) in [
        ("haizijie/middle-school", "海子街中学", 150),
        ("haizijie/baimashan-state-forest", "白马山林场", 149),
    ] {
        let entity = id(key);
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .filter(|e| e.id == entity)
                .count(),
            1
        );
        let row = record(&input, 170, key);
        assert_eq!(row.observation.name, name);
        assert_eq!(
            row.observation.parent_id,
            Some(id("shashao/shaoguan-brigade"))
        );
        assert!(row.observation.coordinate.is_some());
        assert_eq!(record(&input, earlier_page, key).entity_id, entity);
        let shard: serde_json::Value = serde_json::from_slice(
            &output[&PathBuf::from(format!("entities/index/{}.json", entity.shard()))],
        )
        .unwrap();
        let records = shard[entity.0.to_string()]["records"].as_array().unwrap();
        let pages: BTreeSet<_> = records
            .iter()
            .map(|r| r["page_or_sheet"].as_str().unwrap())
            .collect();
        assert_eq!(
            pages,
            BTreeSet::from([earlier_page.to_string().as_str(), "169", "170"])
        );
    }
}

#[test]
fn river_and_mountain_records_connect_adjacent_commune_evidence() {
    let input = input();
    let river = id("haizijie/qiansuo-river");
    for (page, key) in [
        (150, "haizijie/qiansuo-river"),
        (151, "haizijie/qiansuo-river"),
        (165, "haizijie/qiansuo-river/course"),
        (169, "haizijie/qiansuo-river/tributary"),
    ] {
        assert_eq!(record(&input, page, key).entity_id, river);
    }
    for key in [
        "shaogang/lanchong-stream",
        "shaogang/xichong-stream",
        "shashao/mahuangqing-stream",
    ] {
        assert_ne!(id(key), river);
    }
    let meizi = record(&input, 165, "shaogang/meizi-stream/course");
    assert_ne!(meizi.entity_id, id("shaogang/meizigou"));
    assert!(
        meizi
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("流入以河公社")
    );
    let maguo = record(&input, 171, "wangzhang/maguo-river/course");
    assert_ne!(maguo.entity_id, id("wangzhang/maguohe"));
    assert!(
        maguo
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("入汉屯公社")
    );
    assert_eq!(
        record(&input, 171, "shashao/meidongliangzi/height").entity_id,
        record(&input, 170, "shashao/meidongliangzi").entity_id
    );
}

#[test]
fn establishment_years_and_reservoir_construction_retain_source_precision() {
    let input = input();
    for (page, commune, name) in [
        (165, "shaogang", "烧杠公社"),
        (169, "shashao", "沙邵公社"),
        (171, "wangzhang", "王张公社"),
    ] {
        let key = format!("{commune}-commune");
        assert!(names(&input, &key, 1957, TimeSelection::Nominal).is_empty());
        assert_eq!(
            names(&input, &key, 1959, TimeSelection::Nominal),
            BTreeSet::from([name.to_owned()])
        );
        let row = record(&input, page, &format!("{key}/establishment"));
        assert_eq!(row.source_date.as_ref().unwrap().text, "1958年");
    }
    let record = record(&input, 165, "shaogang/xichong-reservoir/construction");
    assert_eq!(record.source_date.as_ref().unwrap().text, "1959年");
    assert!(record.observation.coordinate.is_none());
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|e| e.key == "editorial/bijie/shaogang/xichong-reservoir/construction")
        .unwrap();
    assert_eq!(estimate.basis, vec![record.id]);
    let value = serde_json::to_value(estimate).unwrap();
    assert_eq!(value["value"]["time"]["instant"]["precision"], "year");
}
