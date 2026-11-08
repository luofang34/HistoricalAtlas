use super::*;
use crate::input::load_blocking;

#[cfg(test)]
fn input() -> LedgerInput {
    load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

#[cfg(test)]
fn id(slug: &str) -> EntityId {
    EntityId::from_key(&format!("earth/cn-gz/zhijin/qimo/{slug}")).unwrap()
}

#[cfg(test)]
fn row<'a>(input: &'a LedgerInput, page: u32, slug: &str) -> &'a Record {
    let key = format!("zhijin-gazetteer-scan-b/p{page}/qimo/{slug}");
    input.data.records.iter().find(|r| r.key == key).unwrap()
}

#[test]
fn qimo_table_continuation_preserves_brigade_membership_and_population_disagreements() {
    let input = input();
    let output = compile(&input).unwrap();
    for (brigade, page, count, total, heading) in [
        ("douyan", 30, 8, 1350, 1339),
        ("xingrong", 30, 12, 1447, 1585),
        ("zhongying", 30, 12, 1664, 1664),
        ("mofeng", 31, 9, 1555, 1552),
        ("damuga", 31, 7, 979, 1527),
    ] {
        let parent = id(&format!("{brigade}-brigade"));
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| {
                r.observation.parent_id == Some(parent)
                    && [Some(38), Some(39)].contains(&r.scan_page)
                    && r.entity_id != id("qimo-locality")
            })
            .collect();
        assert_eq!(rows.len(), count, "{brigade}");
        assert_eq!(
            rows.iter()
                .filter_map(|r| r.observation.population)
                .sum::<u64>(),
            total
        );
        let header = row(&input, page, &format!("{brigade}-brigade"));
        assert_eq!(header.observation.population, Some(heading));
        for record in rows {
            let published = tests::published_record(&output, record);
            assert_eq!(published["observation"]["parent_id"], parent.0.to_string());
            assert_eq!(published["scan_page"], record.scan_page.unwrap());
            assert!(published["observation"]["coordinate"].is_null());
        }
    }
    for slug in [
        "wafang-zhai",
        "moshi-chong",
        "huagu-lin",
        "tiansheng-qiao",
        "hebian",
    ] {
        assert_eq!(
            row(&input, 31, slug).observation.parent_id,
            Some(id("zhongying-brigade"))
        );
    }
}

#[test]
fn qimo_locality_note_does_not_duplicate_the_village_population() {
    let input = input();
    let output = compile(&input).unwrap();
    let village = row(&input, 30, "qimo-village");
    let locality = row(&input, 30, "qimo-locality");
    assert_ne!(village.entity_id, locality.entity_id);
    assert_eq!(village.observation.population, Some(211));
    let published = tests::published_record(&output, locality);
    assert!(published["observation"]["population"].is_null());
    assert_eq!(
        published["verbatim_quote"],
        "包括：龙潭田、岩脚、苗水寨、曾家寨"
    );
    assert_eq!(locality.observation.parent_id, Some(id("douyan-brigade")));
    assert_eq!(
        row(&input, 30, "yanjiao").observation.parent_id,
        Some(id("zhongying-brigade"))
    );
    let qingtou = tests::published_record(&output, row(&input, 31, "qingtou"));
    assert!(qingtou["observation"]["population"].is_null());
    assert_eq!(
        qingtou["observation"]["note"],
        "Population cell: an indistinct first digit followed by 7."
    );
}

#[test]
fn differing_map_and_table_spellings_do_not_silently_share_locations() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, table, map) in [
        (30, "meidong-yan", "meidong-zhai"),
        (30, "shuili-chong", "muli-chong"),
        (30, "fuguang-tian", "huguang-tian"),
        (31, "huagu-lin", "huagao-lin"),
        (31, "longtan-riverbank-tian", "longtan-tian"),
        (31, "tunjiao", "chuijiao"),
        (31, "taoshu-bend", "taoshu-wan"),
        (31, "lujia-reed-chong", "lujia-chong"),
        (31, "sichong", "songchong"),
        (31, "azuo-ba", "anzuo-ba"),
    ] {
        let table = row(&input, page, table);
        let map = row(&input, 28, map);
        assert_ne!(table.entity_id, map.entity_id);
        assert_ne!(table.observation.name, map.observation.name);
        assert_eq!(
            tests::published_record(&output, table)["observation"]["name"],
            table.observation.name
        );
        assert!(table.observation.coordinate.is_none());
        assert!(
            input
                .traces
                .iter()
                .flat_map(|sheet| &sheet.places)
                .all(|place| place.record_id != table.id)
        );
    }
}

#[test]
fn qimo_seats_and_printed_pinyin_retain_their_individual_citations() {
    let input = input();
    let output = compile(&input).unwrap();
    for (page, unit, seat) in [
        (30, "douyan-brigade", "zhulin-zhai"),
        (30, "xingrong-brigade", "bana"),
        (30, "zhongying-brigade", "jieshang"),
        (31, "damuga-brigade", "baila-shu"),
        (30, "commune-seat", "chenjia-zhai"),
        (31, "mofeng-table-seat", "longtan-riverbank-tian"),
    ] {
        let record = row(&input, page, unit);
        let published = tests::published_record(&output, record);
        assert_eq!(published["observation"]["seat_id"], id(seat).0.to_string());
        assert_eq!(published["page_or_sheet"], page.to_string());
    }
    assert_eq!(
        row(&input, 28, "mofeng-brigade").observation.seat_id,
        Some(id("jibu"))
    );
    for (page, slug, spelling) in [
        (30, "meidong-yan", "Meídòngzhài"),
        (30, "shuili-chong", "Múlíchōng"),
        (30, "zhongying-brigade", "Zhōngxīng Dàduì"),
        (31, "azuo-ba", "Āzhúoba"),
    ] {
        let published = tests::published_record(&output, row(&input, page, slug));
        assert_eq!(published["observation"]["pinyin"], spelling);
    }
    assert_eq!(
        row(&input, 30, "chongtou-shang").observation.ethnicities,
        ["南"]
    );
}
