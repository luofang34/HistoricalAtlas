use super::*;
use atlas_core::assertion::AdministrativeRank;

#[test]
fn lanni_brigades_and_directly_administered_villages_retain_source_hierarchy() {
    let input = input();
    for (brigade, seat, count) in [
        ("dayakou", "dayakou", 4),
        ("longtang", "qingganglin", 3),
        ("pingshan", "pingshan", 3),
        ("anshan", "daomakan-seat", 5),
    ] {
        let key = format!("lanni/{brigade}-brigade");
        let row = record(&input, 174, &key);
        assert_eq!(row.observation.rank, Some(AdministrativeRank::Brigade));
        assert_eq!(row.observation.seat_id, Some(id(&format!("lanni/{seat}"))));
        let children = input
            .data
            .records
            .iter()
            .filter(|r| r.scan_page == Some(209) && r.observation.parent_id == Some(id(&key)));
        assert_eq!(children.count(), count);
    }
    for (page, commune, count, seat) in [
        (176, "guole", 9, "guoleqiao"),
        (178, "liuhua", 8, "zhongjiazhai"),
        (180, "wangjiaba", 16, "wangjiaba"),
    ] {
        let commune_key = format!("{commune}-commune");
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| r.scan_page == Some(page + 35))
            .collect();
        assert!(
            rows.iter()
                .all(|r| r.observation.rank != Some(AdministrativeRank::Brigade))
        );
        let villages: Vec<_> = rows
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
            .collect();
        assert_eq!(villages.len(), count);
        assert!(
            villages
                .iter()
                .all(|r| r.observation.parent_id == Some(id(&commune_key)))
        );
        assert_eq!(
            record(&input, page, &commune_key).observation.seat_id,
            Some(id(&format!("{commune}/{seat}")))
        );
    }
}

#[test]
fn coincident_and_same_named_places_keep_distinct_identities() {
    let input = input();
    for (page, left, right) in [
        (174, "lanni/daomakan-seat", "lanni/daomakan-east"),
        (174, "lanni/banbianqing", "lanni/banbian-mountain"),
        (174, "lanni/yinggezui", "lanni/yingge-mountain"),
        (176, "guole/guoleqiao", "guole/guole-bridge"),
    ] {
        let a = record(&input, page, left);
        let b = record(&input, page, right);
        assert_eq!(a.observation.name, b.observation.name);
        assert_ne!(a.entity_id, b.entity_id);
    }
    let west = record(&input, 174, "lanni/daomakan-seat");
    let east = record(&input, 174, "lanni/daomakan-east");
    assert_eq!(west.observation.population, Some(311));
    assert!(east.observation.population.is_none());
    assert_eq!(
        west.observation.coordinate.as_ref().unwrap().longitude_text,
        "105°25′"
    );
    assert_eq!(
        east.observation.coordinate.as_ref().unwrap().longitude_text,
        "105°26′"
    );
    let ridge = record(&input, 175, "guole/wangpo-ridge/height");
    assert_eq!(
        ridge.observation.name,
        record(&input, 176, "guole/wangpoling").observation.name
    );
    assert_ne!(ridge.entity_id, id("guole/wangpoling"));
    assert_ne!(ridge.entity_id, id("lanni/duqing-mountain"));
    assert!(ridge.observation.coordinate.is_none());
}

#[test]
fn differing_seat_and_population_claims_are_published_together() {
    let input = input();
    let output = compile(&input).unwrap();
    let entity = id("guole-commune");
    let shard: serde_json::Value = serde_json::from_slice(
        &output[&PathBuf::from(format!("entities/index/{}.json", entity.shard()))],
    )
    .unwrap();
    let seats: BTreeSet<_> = shard[entity.0.to_string()]["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["observation"]["seat_id"].as_str())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        seats,
        BTreeSet::from([
            id("guole/liujiawan").0.to_string(),
            id("guole/guoleqiao").0.to_string(),
        ])
    );
    assert_eq!(
        record(&input, 177, "liuhua-commune/population")
            .observation
            .population,
        Some(3302)
    );
    assert_eq!(
        record(&input, 178, "liuhua-commune").observation.population,
        Some(3213)
    );
    let small = record(&input, 178, "liuhua/dajianchong");
    assert!(small.observation.population.is_none());
    assert!(small.observation.ethnicities.is_empty());
    assert_eq!(
        small.observation.note.as_deref(),
        Some("村，人口并入刘华大寨")
    );
    assert_eq!(small.observation.parent_id, Some(id("liuhua-commune")));
    assert_eq!(
        record(&input, 178, "liuhua/liuhua-dazhai")
            .observation
            .population,
        Some(1086)
    );
    let lanni_sum: u64 = input
        .data
        .records
        .iter()
        .filter(|r| {
            r.scan_page == Some(209) && r.observation.rank == Some(AdministrativeRank::Brigade)
        })
        .filter_map(|r| r.observation.population)
        .sum();
    assert_eq!(lanni_sum, 3923);
    assert_eq!(
        record(&input, 174, "lanni-commune").observation.population,
        Some(4012)
    );
}

#[test]
fn river_names_share_only_the_identity_established_by_the_source() {
    let input = input();
    let guole = record(&input, 175, "haizijie/qiansuo-river/course");
    let qiansuo = record(&input, 176, "haizijie/qiansuo-river");
    assert_eq!(guole.observation.name, "果乐河");
    assert_eq!(qiansuo.observation.name, "前所河");
    assert_eq!(guole.entity_id, qiansuo.entity_id);
    assert!(qiansuo.observation.coordinate.is_none());
    assert!(
        qiansuo
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("3.5公里")
    );
    let liuhua = record(&input, 178, "haizijie/haogou-river");
    let wangjiaba = record(&input, 180, "haizijie/haogou-river");
    assert_eq!(liuhua.entity_id, wangjiaba.entity_id);
    assert!(
        liuhua
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("约6公里")
    );
    assert!(
        wangjiaba
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("3公里")
    );
    assert!(liuhua.observation.coordinate.is_none());
    assert!(wangjiaba.observation.coordinate.is_none());
    assert_ne!(
        liuhua.entity_id,
        record(&input, 177, "liuhua/liuhua-river/course").entity_id
    );
}

#[test]
fn source_spellings_and_unquantified_tree_age_are_retained() {
    let input = input();
    for (page, key, pinyin) in [
        (178, "liuhua/zhongjiazhai", "Zhōngjiāzhài"),
        (178, "liuhua/dajianchong", "Dàjiǎnchǒng"),
        (180, "wangjiaba/duiwo-shuijing", "Duìwōshuǐjǐng"),
    ] {
        assert_eq!(
            record(&input, page, key).observation.pinyin.as_deref(),
            Some(pinyin)
        );
    }
    assert_eq!(
        record(&input, 174, "lanni/yingge-mountain")
            .observation
            .note
            .as_deref(),
        Some("山峰、海拔1719,9米")
    );
    let tree = record(&input, 180, "wangjiaba/potoushang-ginkgo");
    assert_eq!(tree.observation.parent_id, Some(id("wangjiaba/potoushang")));
    assert_eq!(
        tree.observation.note.as_deref(),
        Some("在坡头上村，树无法考证。")
    );
    assert!(
        record(&input, 179, "wangjiaba/potoushang-ginkgo/age")
            .verbatim_quote
            .as_ref()
            .unwrap()
            .contains("近五百年")
    );
    assert!(
        !input
            .data
            .estimates
            .iter()
            .any(|e| e.entity_id == tree.entity_id)
    );
    assert_ne!(tree.entity_id, id("wangjiaba/baiguoshu"));
    assert_eq!(
        record(&input, 178, "liuhua/liuhua-temple")
            .observation
            .note
            .as_deref(),
        Some("现已废")
    );
}

#[test]
fn commune_dates_are_bounded_without_redating_rebuilt_bridges() {
    let input = input();
    for (page, commune, name, date) in [
        (173, "lanni", "滥泥公社", "1958年"),
        (175, "guole", "果乐公社", "58年"),
        (177, "liuhua", "刘华公社", "58年"),
        (179, "wangjiaba", "王家坝公社", "1958年"),
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
            date
        );
    }
    let rebuilt = record(&input, 175, "guole/guole-bridge/reconstruction");
    assert_eq!(rebuilt.source_date.as_ref().unwrap().text, "1975年");
    assert!(
        !input
            .data
            .estimates
            .iter()
            .any(|e| e.entity_id == rebuilt.entity_id)
    );
    let old_name = &record(&input, 175, "guole/guole-bridge/name-history")
        .observation
        .former_names[0];
    assert_eq!(old_name.name, "倮罗桥");
    assert_eq!(old_name.source_date.as_ref().unwrap().text, "嘉庆（清）");
    assert!(old_name.validity.is_none());
}
