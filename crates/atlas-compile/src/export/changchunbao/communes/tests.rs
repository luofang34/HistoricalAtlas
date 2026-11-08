use super::*;

#[test]
fn continued_brigades_keep_blank_values_and_source_population_disagreements() {
    let input = input();
    let maoping = record(&input, 116, "huangnichong/maoping-brigade");
    for key in ["zhaojia-chongzi", "zhengjia-chongzi", "maoping", "aotang"] {
        assert_eq!(
            record(&input, 117, &format!("huangnichong/{key}"))
                .observation
                .parent_id,
            Some(maoping.entity_id)
        );
    }
    let bailong = record(&input, 117, "huangnichong/bailong-brigade");
    assert!(bailong.observation.population.is_none());
    for (commune, page, overview, table) in [
        ("huangnichong", 115, 5580, 5408),
        ("shaba", 119, 7915, 7916),
    ] {
        let key = format!("{commune}-commune");
        let population = input
            .data
            .records
            .iter()
            .find(|row| row.key == format!("bijie-gazetteer/p{page}/{key}/population"))
            .unwrap();
        assert_eq!(population.observation.population, Some(overview));
        assert_eq!(
            record(&input, page + 1, &key).observation.population,
            Some(table)
        );
        assert!(population.source_date.is_none());
    }
    assert_eq!(
        record(&input, 120, "shaba/longjiazhai")
            .observation
            .ethnicities,
        vec!["南京"]
    );
}

#[test]
fn table_spellings_and_coordinates_survive_without_editorial_correction() {
    let input = input();
    for (page, key, name, pinyin) in [
        (112, "wangfeng/xiawuxi", "下伍溪", "Xiàwǔqī"),
        (112, "wangfeng/sandaoshui", "三倒水", "Sāndǎoshuǐ"),
        (116, "huangnichong/beishan", "北山", "Bǎishān"),
        (116, "huangnichong/jiadanwan", "柳担湾", "Jiādānwān"),
        (117, "huangnichong/aotang", "凹塘", "Āàtáng"),
        (120, "shaba/anjiaying", "安家营", "Āajiāyíng"),
        (121, "shaba/chuandong", "穿洞", "Chuādòng"),
    ] {
        let observation = &record(&input, page, key).observation;
        assert_eq!(observation.name, name);
        assert_eq!(observation.pinyin.as_deref(), Some(pinyin));
    }
    let bridge = record(&input, 121, "shaba/xinqiao")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(bridge.latitude_text, "27°13");
    assert_eq!(bridge.point.latitude, 27.0 + 13.0 / 60.0);
    assert_eq!(bridge.resolution_arcminutes, 1.0);
    let reservoir = record(&input, 113, "wangfeng/zhangguan-reservoir")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert_eq!(reservoir.longitude_text, "105°18′");
    assert_eq!(reservoir.point.longitude, 105.0 + 18.0 / 60.0);
    assert!(
        record(&input, 111, "wangfeng-commune")
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("至105°15′")
    );
}

#[test]
fn caves_places_and_road_facilities_have_independent_identities() {
    let input = input();
    for (first_page, second_page, first, second, kind) in [
        (
            112,
            113,
            "wangfeng/daodong",
            "wangfeng/daodong-cave",
            EntityKind::Cave,
        ),
        (
            117,
            117,
            "huangnichong/yejidong-place",
            "huangnichong/yejidong-cave",
            EntityKind::Cave,
        ),
        (
            121,
            121,
            "shaba/zhujia-yandong-place",
            "shaba/zhujia-yandong-cave",
            EntityKind::Cave,
        ),
        (
            120,
            121,
            "shaba/gaoshanpu",
            "shaba/gaoshanpu-daoban",
            EntityKind::Facility,
        ),
        (
            120,
            121,
            "shaba/muguachong",
            "shaba/muguachong-daoban",
            EntityKind::Facility,
        ),
    ] {
        let first = record(&input, first_page, first);
        let second = record(&input, second_page, second);
        assert_ne!(first.entity_id, second.entity_id);
        assert_eq!(first.observation.coordinate, second.observation.coordinate);
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|entity| entity.id == second.entity_id)
                .unwrap()
                .kind,
            kind
        );
    }
    let huangni = record(&input, 117, "huangnichong/maoping");
    let shaba = record(&input, 120, "shaba/maoping");
    assert_eq!(huangni.observation.name, shaba.observation.name);
    assert_ne!(huangni.entity_id, shaba.entity_id);
    assert_ne!(huangni.observation.coordinate, shaba.observation.coordinate);
}

#[test]
fn shared_rivers_retain_each_headwater_name_and_local_reach() {
    let input = input();
    for (page, text) in [(115, "北山"), (117, "白山")] {
        let river = record(&input, page, "daotianhe-river");
        assert!(river.observation.extent.as_ref().unwrap().contains(text));
        assert!(river.observation.coordinate.is_none());
    }
    for page in [25, 31, 39] {
        assert_eq!(
            record(&input, page, "daotianhe-river").entity_id,
            record(&input, 117, "daotianhe-river").entity_id
        );
    }
    let upstream = record(&input, 119, "gezuo/gezuo-river");
    let downstream = record(&input, 103, "gezuo/gezuo-river");
    assert_eq!(upstream.entity_id, downstream.entity_id);
    assert_eq!(upstream.observation.name, "干河沟");
    assert_eq!(downstream.observation.name, "戈座河");
    assert!(
        upstream
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("三公里许")
    );
    assert!(
        downstream
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("五公里")
    );
    assert_ne!(
        upstream.observation.parent_id,
        downstream.observation.parent_id
    );
    for key in ["longqi-reservoir", "wachang-reservoir"] {
        let reservoir = record(&input, 111, &format!("wangfeng/{key}"));
        assert!(reservoir.source_date.is_none());
        assert!(reservoir.observation.coordinate.is_none());
        assert!(
            input
                .data
                .estimates
                .iter()
                .all(|estimate| estimate.entity_id != reservoir.entity_id)
        );
    }
}

#[test]
fn commune_mergers_and_renamings_query_their_cited_years() {
    let input = input();
    for (key, year, before, after) in [
        ("wangfeng-commune", 1966, "王丰人民公社", "王丰公社"),
        ("huangnichong-commune", 1981, "黄泥公社", "黄泥冲公社"),
    ] {
        assert_eq!(
            names(&input, key, year - 1, TimeSelection::Nominal),
            BTreeSet::from([before.into()])
        );
        assert_eq!(
            names(&input, key, year + 1, TimeSelection::Nominal),
            BTreeSet::from([after.into()])
        );
        assert_eq!(
            names(&input, key, year, TimeSelection::Possible),
            BTreeSet::from([before.into(), after.into()])
        );
        assert!(names(&input, key, year, TimeSelection::Certain).is_empty());
    }
    assert!(names(&input, "shaba-commune", 1965, TimeSelection::Possible).is_empty());
    assert_eq!(
        names(&input, "shaba-commune", 1967, TimeSelection::Nominal),
        BTreeSet::from(["沙坝公社".into()])
    );
    for (key, name) in [("shale", "沙乐公社"), ("gaoshanpu", "高山铺公社")] {
        let former = format!("shaba/{key}-former-commune");
        assert!(names(&input, &former, 1957, TimeSelection::Possible).is_empty());
        assert_eq!(
            names(&input, &former, 1965, TimeSelection::Certain),
            BTreeSet::from([name.into()])
        );
        assert!(names(&input, &former, 1967, TimeSelection::Possible).is_empty());
        assert!(record(&input, 119, &former).observation.parent_id.is_none());
    }
}

#[test]
fn road_opening_and_predecessors_do_not_backdate_commune_establishment() {
    let input = input();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    for (key, year) in [
        ("wangfeng-commune", 1958),
        ("huangnichong-commune", 1958),
        ("shaba-commune", 1966),
        ("shaba/shale-former-commune", 1958),
        ("shaba/gaoshanpu-former-commune", 1958),
    ] {
        for (year, expected) in [(year - 1, 0), (year + 1, 1)] {
            let slice = timeline.at(instant(year), TimeSelection::Certain).unwrap();
            let origins = slice
                .entities
                .iter()
                .find(|entity| entity.entity_id == id(key))
                .map(|entity| entity.origins.len())
                .unwrap_or_default();
            assert_eq!(origins, expected, "{key}, {year}");
        }
    }
    let transfer = input
        .data
        .records
        .iter()
        .find(|row| row.key == "bijie-gazetteer/p115/huangnichong-commune/seat-transfer")
        .unwrap();
    assert_eq!(transfer.source_date.as_ref().unwrap().text, "1956年");
    assert_eq!(
        transfer.observation.seat_id,
        Some(id("huangnichong/beishan"))
    );
    assert!(
        input
            .data
            .estimates
            .iter()
            .all(|estimate| !estimate.basis.contains(&transfer.id))
    );
    for key in ["zhangguantun", "xiawuxi"] {
        let former = record(&input, 111, &format!("wangfeng/{key}-former-commune"));
        assert_ne!(former.entity_id, id(&format!("wangfeng/{key}")));
        assert!(
            input
                .data
                .estimates
                .iter()
                .all(|estimate| estimate.entity_id != former.entity_id)
        );
    }
}
