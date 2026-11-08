use super::*;
use atlas_core::assertion::AdministrativeRank;

#[test]
fn tianbaqiao_brigade_annotations_do_not_assign_unstated_village_membership() {
    let input = input();
    let commune = id("tianbaqiao-commune");
    let rows: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|r| r.scan_page == Some(194))
        .collect();
    let brigades: Vec<_> = rows
        .iter()
        .filter(|r| r.observation.rank == Some(AdministrativeRank::Brigade))
        .collect();
    assert_eq!(brigades.len(), 4);
    for (key, population, note) in [
        ("shazidi-brigade", 313, "大队"),
        ("shierwan-brigade", 170, "大队"),
        ("zhebaotian-brigade", 215, "大队"),
        ("wafang-brigade", 233, "公社驻地(大队)"),
    ] {
        let row = record(&input, 159, &format!("tianbaqiao/{key}"));
        assert_eq!(row.observation.population, Some(population));
        assert_eq!(row.observation.note.as_deref(), Some(note));
        assert_eq!(row.observation.parent_id, Some(commune));
        assert!(row.observation.seat_id.is_none());
    }
    assert_eq!(
        record(&input, 159, "tianbaqiao-commune")
            .observation
            .seat_id,
        Some(id("tianbaqiao/wafang-brigade"))
    );
    let children: Vec<_> = rows.iter().filter(|r| r.entity_id != commune).collect();
    assert_eq!(children.len(), 36);
    assert!(
        children
            .iter()
            .all(|r| r.observation.parent_id == Some(commune))
    );
    assert_eq!(
        record(&input, 158, "tianbaqiao-commune/subdivisions")
            .verbatim_quote
            .as_deref(),
        Some("直辖36个生产队，32个自然村。")
    );
}

#[test]
fn pangxie_table_continuation_retains_all_four_brigades_and_forty_four_villages() {
    let input = input();
    let mut villagers = BTreeSet::new();
    for (page, key, seat, count) in [
        (162, "pangxie", "pangxiejing", 8),
        (162, "xiushuijing", "xiushuijing", 15),
        (162, "xiaohuidong", "xiaohuidong", 11),
        (163, "gaoqiao", "gaoqiao", 10),
    ] {
        let brigade = id(&format!("pangxie/{key}-brigade"));
        assert_eq!(
            record(&input, page, &format!("pangxie/{key}-brigade"))
                .observation
                .seat_id,
            Some(id(&format!("pangxie/{seat}")))
        );
        let children: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| r.observation.parent_id == Some(brigade))
            .collect();
        assert_eq!(children.len(), count, "{key}");
        for row in children {
            assert!(villagers.insert(row.entity_id));
            assert_eq!(
                input
                    .data
                    .entities
                    .iter()
                    .find(|e| e.id == row.entity_id)
                    .unwrap()
                    .kind,
                EntityKind::Settlement
            );
        }
    }
    assert_eq!(villagers.len(), 44);
    for key in ["dawan", "shanbeihou"] {
        assert_eq!(
            record(&input, 163, &format!("pangxie/{key}"))
                .observation
                .parent_id,
            Some(id("pangxie/xiaohuidong-brigade"))
        );
    }
    assert_eq!(
        record(&input, 162, "pangxie-commune").observation.seat_id,
        Some(id("pangxie/pangxiejing"))
    );
}

#[test]
fn missing_and_aggregated_populations_stay_distinct_from_reported_counts() {
    let input = input();
    for key in ["bainitang", "laoyayan"] {
        let row = record(&input, 162, &format!("pangxie/{key}"));
        assert!(row.observation.population.is_none());
        assert!(row.observation.ethnicities.is_empty());
        assert!(row.observation.coordinate.is_some());
    }
    assert_eq!(
        record(&input, 162, "pangxie/laoyayan")
            .observation
            .note
            .as_deref(),
        Some("人口计入大岩洞")
    );
    assert_eq!(
        record(&input, 162, "pangxie/dayandong")
            .observation
            .population,
        Some(154)
    );
    assert_eq!(
        record(&input, 162, "pangxie-commune")
            .observation
            .population,
        Some(6844)
    );
    assert_eq!(
        record(&input, 162, "pangxie-commune")
            .observation
            .ethnicities,
        ["汉", "苗", "彝", "满", "布依"]
    );
    assert_eq!(
        record(&input, 161, "pangxie-commune/population")
            .observation
            .households,
        Some(1187)
    );
    assert!(
        record(&input, 162, "pangxie-commune")
            .observation
            .households
            .is_none()
    );
    let total: u64 = input
        .data
        .records
        .iter()
        .filter(|r| {
            [Some(197), Some(198)].contains(&r.scan_page)
                && r.observation.rank == Some(AdministrativeRank::Brigade)
        })
        .filter_map(|r| r.observation.population)
        .sum();
    assert_eq!(total, 6568);
}

#[test]
fn mountains_toponyms_and_settlements_preserve_their_separate_rows() {
    let input = input();
    for (first, second, kind) in [
        ("daheishan", "dahei-mountain", EntityKind::Settlement),
        ("shiziyan-place", "shizi-mountain", EntityKind::Toponym),
    ] {
        let a = record(&input, 159, &format!("tianbaqiao/{first}"));
        let b = record(&input, 160, &format!("tianbaqiao/{second}"));
        assert_ne!(a.entity_id, b.entity_id);
        assert_eq!(a.observation.name, b.observation.name);
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|e| e.id == a.entity_id)
                .unwrap()
                .kind,
            kind
        );
        assert_eq!(
            input
                .data
                .entities
                .iter()
                .find(|e| e.id == b.entity_id)
                .unwrap()
                .kind,
            EntityKind::Mountain
        );
    }
    for (key, note) in [
        ("shizi-mountain", "山峰，海拔175.8米"),
        ("shuiniu-mountain", "山峰，海拔197.6米"),
        ("shaofeng-cave", "洞，5/4000"),
    ] {
        assert_eq!(
            record(&input, 160, &format!("tianbaqiao/{key}"))
                .observation
                .note
                .as_deref(),
            Some(note)
        );
    }
    assert_eq!(
        record(&input, 159, "tianbaqiao/gaogengzi")
            .observation
            .pinyin
            .as_deref(),
        Some("Gāngěngzǐ")
    );
    assert_eq!(
        record(&input, 163, "pangxie/yiwanshui")
            .observation
            .pinyin
            .as_deref(),
        Some("Yìwǎoshuǐ")
    );
    assert_eq!(
        record(&input, 163, "pangxie/shikala").observation.name,
        "石㮟㮟"
    );
}

#[test]
fn named_river_confluence_keeps_the_tributary_separate_from_the_main_river() {
    let input = input();
    let tributary = record(&input, 158, "tianbaqiao/tianba-river/course");
    let main = record(&input, 160, "guanyinqiao-river");
    assert_ne!(tributary.entity_id, main.entity_id);
    assert_eq!(main.entity_id, id("guanyinqiao-river"));
    assert!(
        tributary
            .observation
            .extent
            .as_ref()
            .unwrap()
            .contains("河水于犀牛塘流入观音桥河")
    );
    assert!(
        main.observation
            .note
            .as_ref()
            .unwrap()
            .contains("源于螃蟹井经田坝桥")
    );
    assert!(tributary.observation.coordinate.is_none());
    assert!(main.observation.coordinate.is_none());
    let pages: BTreeSet<_> = input
        .data
        .records
        .iter()
        .filter(|r| r.entity_id == main.entity_id)
        .map(|r| r.page_or_sheet.as_deref().unwrap())
        .collect();
    assert_eq!(pages, BTreeSet::from(["31", "156", "157", "160"]));
}

#[test]
fn commune_establishment_uses_the_cited_year_and_retains_original_date_text() {
    let input = input();
    for (page, key, name, text) in [
        (158, "tianbaqiao-commune", "田坝桥公社", "58年"),
        (161, "pangxie-commune", "螃蟹人民公社", "1958年"),
    ] {
        assert!(names(&input, key, 1957, TimeSelection::Nominal).is_empty());
        assert_eq!(
            names(&input, key, 1959, TimeSelection::Nominal),
            BTreeSet::from([name.to_owned()])
        );
        let row = record(&input, page, &format!("{key}/establishment"));
        assert_eq!(row.source_date.as_ref().unwrap().text, text);
        assert!(row.observation.coordinate.is_none());
        let estimate = input
            .data
            .estimates
            .iter()
            .find(|e| e.key == format!("editorial/bijie/{key}/establishment"))
            .unwrap();
        assert_eq!(estimate.basis, vec![row.id]);
        let serialized = serde_json::to_value(estimate).unwrap();
        assert_eq!(serialized["value"]["time"]["instant"]["precision"], "year");
    }
}
