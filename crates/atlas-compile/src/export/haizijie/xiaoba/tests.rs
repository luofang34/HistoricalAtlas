use super::*;
use atlas_core::assertion::AdministrativeRank;

#[test]
fn brigade_members_and_seats_follow_each_complete_table() {
    let input = input();
    for (page, commune, brigade, seat, count) in [
        (182, "xiaoba", "xiaoba", "xiaoba", 3),
        (182, "xiaoba", "fenghuang", "dashan", 4),
        (182, "xiaoba", "xichong", "xichong", 4),
        (182, "xiaoba", "guanjing", "guanjingshang", 3),
        (182, "xiaoba", "dongshang", "dongshang", 1),
        (182, "xiaoba", "taxing", "dagoubian", 4),
        (182, "xiaoba", "xinchang", "xinchangjie", 2),
        (182, "xiaoba", "huangnidi", "huangni", 1),
        (186, "hantun", "hantun", "hantun", 1),
        (186, "hantun", "shuitang", "shuitang", 3),
        (186, "hantun", "guanba", "guanba", 2),
        (186, "hantun", "liangjiazhai", "liangjiazhai", 2),
        (188, "haogou", "haogou", "haogou", 6),
        (188, "haogou", "hewei", "heweiba", 6),
    ] {
        let key = format!("{commune}/{brigade}-brigade");
        let source = record(&input, page, &key);
        assert_eq!(source.observation.rank, Some(AdministrativeRank::Brigade));
        assert_eq!(
            source.observation.parent_id,
            Some(id(&format!("{commune}-commune")))
        );
        assert_eq!(
            source.observation.seat_id,
            Some(id(&format!("{commune}/{seat}")))
        );
        assert_eq!(
            input
                .data
                .records
                .iter()
                .filter(|r| {
                    r.scan_page == Some(page + 35) && r.observation.parent_id == Some(id(&key))
                })
                .count(),
            count
        );
    }
    let direct: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|r| {
            r.scan_page == Some(225) && r.observation.parent_id == Some(id("zhongtun-commune"))
        })
        .collect();
    assert_eq!(direct.len(), 9);
    assert!(direct.iter().all(|r| r.observation.rank.is_none()));
    for (page, commune) in [
        (182, "xiaoba"),
        (186, "hantun"),
        (188, "haogou"),
        (190, "zhongtun"),
    ] {
        assert_eq!(
            record(&input, page, &format!("{commune}-commune"))
                .observation
                .seat_id,
            Some(id(&format!("{commune}/{commune}")))
        );
    }
}

#[test]
fn anomalous_coordinates_keep_printed_degrees_and_minutes() {
    let input = input();
    for (page, key, longitude, latitude) in [
        (182, "xiaoba/fenghuang-mountain", "105°22′", "27°19′"),
        (186, "hantun-commune", "107°27′", "27°20′"),
        (186, "hantun/hantun", "105°27′", "25°20′"),
        (188, "haogou-commune", "105°20′", "27°20′"),
        (188, "haogou/shizishan", "105°27′", "25°18′"),
    ] {
        let coordinate = record(&input, page, key)
            .observation
            .coordinate
            .as_ref()
            .unwrap();
        assert_eq!(coordinate.longitude_text, longitude);
        assert_eq!(coordinate.latitude_text, latitude);
        assert_eq!(coordinate.resolution_arcminutes, 1.0);
    }
    let hantun = record(&input, 186, "hantun-commune")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert!((hantun.point.longitude - 107.45).abs() < 1e-12);
    let village = record(&input, 186, "hantun/hantun")
        .observation
        .coordinate
        .as_ref()
        .unwrap();
    assert!((village.point.latitude - (25.0 + 20.0 / 60.0)).abs() < 1e-12);
    assert!(
        record(&input, 185, "hantun-commune/extent")
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("105°27′至105°28′")
    );
}

#[test]
fn missing_populations_and_population_disagreements_survive_export() {
    let input = input();
    let north = record(&input, 182, "xiaoba/heshangchong-north");
    let south = record(&input, 182, "xiaoba/heshangchong-south");
    assert_eq!(north.observation.name, south.observation.name);
    assert_ne!(north.entity_id, south.entity_id);
    assert_eq!(north.observation.population, Some(65));
    assert!(south.observation.population.is_none());
    assert!(south.observation.ethnicities.is_empty());
    assert_ne!(
        north.observation.coordinate.as_ref().unwrap().latitude_text,
        south.observation.coordinate.as_ref().unwrap().latitude_text
    );
    for (page, commune, summed, total) in [(186, "hantun", 3904, 3897), (188, "haogou", 5711, 5875)]
    {
        let sum: u64 = input
            .data
            .records
            .iter()
            .filter(|r| {
                r.scan_page == Some(page + 35)
                    && r.observation.rank == Some(AdministrativeRank::Brigade)
            })
            .filter_map(|r| r.observation.population)
            .sum();
        assert_eq!(sum, summed);
        assert_eq!(
            record(&input, page, &format!("{commune}-commune"))
                .observation
                .population,
            Some(total)
        );
    }
    let census = record(&input, 181, "xiaoba-commune/population");
    assert_eq!(census.source_date.as_ref().unwrap().text, "1982年");
    assert_eq!(census.observation.households, Some(1745));
    assert_eq!(census.observation.population, Some(9174));
    assert!(record(&input, 182, "xiaoba-commune").source_date.is_none());
    let output = compile(&input).unwrap();
    let shard: serde_json::Value = serde_json::from_slice(
        &output[&PathBuf::from(format!("entities/index/{}.json", south.entity_id.shard()))],
    )
    .unwrap();
    let published = shard[south.entity_id.0.to_string()]["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == south.id.0.to_string())
        .unwrap();
    assert!(published["observation"]["population"].is_null());
}

#[test]
fn river_courses_share_cited_identities_without_invented_point_coordinates() {
    let input = input();
    assert_eq!(
        record(&input, 185, "wangzhang/maguo-river/course").entity_id,
        record(&input, 171, "wangzhang/maguo-river/course").entity_id
    );
    for page in [178, 180, 188] {
        assert_eq!(
            record(&input, page, "haizijie/haogou-river").entity_id,
            id("haizijie/haogou-river")
        );
    }
    for (page, key, fragment) in [
        (183, "xiaoba/xinchang-river", "约5公里"),
        (185, "wangzhang/maguo-river/course", "一公里"),
        (188, "haizijie/haogou-river", "经刘华、豪沟"),
        (189, "zhongtun/zhongtun-river/course", "大方归化河"),
        (190, "zhongtun/zhongtun-river", "汇入保河"),
    ] {
        let row = record(&input, page, key);
        assert!(row.observation.coordinate.is_none());
        assert!(row.observation.extent.as_ref().unwrap().contains(fragment));
    }
    assert_ne!(id("xiaoba/xinchang-river"), id("haizijie/haogou-river"));
    assert_ne!(
        record(&input, 188, "haogou/shizishan").entity_id,
        record(&input, 188, "haogou/shizi-mountain").entity_id
    );
}

#[test]
fn historical_names_and_district_abolition_do_not_redate_commune_establishment() {
    let input = input();
    for (page, commune, name) in [
        (181, "xiaoba", "小坝公社"),
        (185, "hantun", "汉屯公社"),
        (187, "haogou", "豪沟公社"),
        (189, "zhongtun", "中屯公社"),
    ] {
        let key = format!("{commune}-commune");
        assert!(names(&input, &key, 1957, TimeSelection::Nominal).is_empty());
        assert_eq!(
            names(&input, &key, 1959, TimeSelection::Nominal),
            BTreeSet::from([name.to_owned()])
        );
        assert_eq!(
            record(&input, page, &format!("{key}/establishment"))
                .source_date
                .as_ref()
                .unwrap()
                .text,
            "1958年"
        );
    }
    let district = record(&input, 185, "hantun-district/abolition");
    assert_eq!(district.source_date.as_ref().unwrap().text, "1957年");
    assert_ne!(district.entity_id, id("hantun-commune"));
    assert_eq!(
        district.observation.rank,
        Some(AdministrativeRank::District)
    );
    for (page, key, expected) in [
        (181, "xiaoba/xiaoba/name-history", vec!["杨柳街", "小坝屯"]),
        (185, "hantun/hantun/name-history", vec!["旱屯"]),
        (
            189,
            "zhongtun/zhongtun/name-history",
            vec!["阿冲屯", "窝中屯"],
        ),
    ] {
        let former = &record(&input, page, key).observation.former_names;
        assert_eq!(
            former.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
            expected
        );
        assert!(former.iter().all(|n| n.validity.is_none()));
    }
}

#[test]
fn sites_trees_and_source_spellings_keep_their_recorded_attributes() {
    let input = input();
    for (page, key, pinyin) in [
        (188, "haogou/jiaoshuwa", "Jiānshùwā"),
        (190, "zhongtun/baiyanglin", "Béiyánglín"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    assert_eq!(
        record(&input, 183, "xiaoba/chuanzhu-temple")
            .observation
            .note
            .as_deref(),
        Some("现为公社粮站。")
    );
    for key in ["xia-temple", "zhongjian-temple", "tingzi", "shita"] {
        assert_eq!(
            record(&input, 183, &format!("xiaoba/{key}"))
                .observation
                .note
                .as_deref(),
            Some("现已废")
        );
    }
    assert_eq!(
        record(&input, 186, "hantun/hantun-bridge")
            .observation
            .note
            .as_deref(),
        Some("桥 (28—6)/15")
    );
    assert_eq!(
        record(&input, 188, "haogou/haogou-bridge")
            .observation
            .note
            .as_deref(),
        Some("(14—6)/15")
    );
    for key in ["ginkgo", "camphor-tree"] {
        let tree = record(&input, 189, &format!("zhongtun/{key}/protection"));
        assert_eq!(tree.observation.parent_id, Some(id("zhongtun/zhongtun")));
        assert!(
            tree.verbatim_quote
                .as_ref()
                .unwrap()
                .contains("县级文物保护单位")
        );
        assert!(
            input
                .data
                .estimates
                .iter()
                .all(|e| e.entity_id != tree.entity_id)
        );
    }
    assert_ne!(id("zhongtun/ginkgo"), id("zhongtun/camphor-tree"));
    assert_ne!(id("zhongtun/ginkgo"), id("wangjiaba/potoushang-ginkgo"));
    assert!(
        record(&input, 189, "zhongtun/camphor-tree/protection")
            .observation
            .coordinate
            .is_none()
    );
}
