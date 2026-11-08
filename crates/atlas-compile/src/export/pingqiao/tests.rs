use super::*;
use crate::input::load_blocking;

#[cfg(test)]
fn input() -> LedgerInput {
    load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

#[cfg(test)]
fn id(slug: &str) -> EntityId {
    EntityId::from_key(&format!("earth/cn-gz/zhijin/pingqiao/{slug}")).unwrap()
}

#[cfg(test)]
fn row<'a>(input: &'a LedgerInput, page: u32, slug: &str) -> &'a Record {
    let key = format!("zhijin-gazetteer-scan-b/p{page}/pingqiao/{slug}");
    input.data.records.iter().find(|r| r.key == key).unwrap()
}

#[test]
fn pingqiao_tables_publish_all_villages_with_source_population_disagreements() {
    let input = input();
    let output = compile(&input).unwrap();
    for (brigade, count, sum, heading, seat) in [
        ("pingqiao", 6, 974, 1047, "laowu-ji"),
        ("zhongba", 10, 1280, 1326, "ganba"),
        ("yangliu", 10, 981, 1017, "yangliu-he"),
        ("sanjia", 5, 965, 1004, "dashi-ban"),
    ] {
        let parent = id(&format!("{brigade}-brigade"));
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| r.scan_page == Some(35) && r.observation.parent_id == Some(parent))
            .collect();
        assert_eq!(rows.len(), count);
        assert_eq!(
            rows.iter()
                .filter_map(|r| r.observation.population)
                .sum::<u64>(),
            sum
        );
        let header = row(&input, 27, &format!("{brigade}-brigade"));
        assert_eq!(header.observation.population, Some(heading));
        assert_eq!(header.observation.seat_id, Some(id(seat)));
        for record in rows {
            let published = tests::published_record(&output, record);
            assert_eq!(published["observation"]["parent_id"], parent.0.to_string());
            assert_eq!(published["page_or_sheet"], "27");
            assert_eq!(published["scan_page"], 35);
            assert!(published["observation"]["coordinate"].is_null());
        }
    }
    let overview = tests::published_record(&output, row(&input, 26, "overview"));
    assert_eq!(overview["observation"]["population"], 4394);
    assert_eq!(overview["observation"]["households"], 891);
    assert_eq!(overview["source_date"]["text"], "一九八一年九月十四日");
}

#[test]
fn pingqiao_map_and_table_names_keep_their_citations_and_symbol_links() {
    let input = input();
    let output = compile(&input).unwrap();
    let table = row(&input, 27, "kengkeng-tian");
    let map = row(&input, 25, "luokeng-tian");
    assert_ne!(table.entity_id, map.entity_id);
    assert_eq!(table.observation.name, "坑坑田");
    assert_eq!(map.observation.name, "落坑田");
    let trace = input.traces.iter().find(|t| t.scan_page == 33).unwrap();
    assert!(trace.places.iter().any(|p| p.record_id == map.id));
    assert!(trace.places.iter().all(|p| p.record_id != table.id));
    let table = row(&input, 27, "sanjia-brigade");
    let map = row(&input, 25, "sanjia-brigade");
    assert_eq!(table.entity_id, map.entity_id);
    assert_eq!(table.observation.name, "三架大队");
    assert_eq!(map.observation.name, "三架山大队");
    assert_eq!(map.observation.seat_id, table.observation.seat_id);
    let coordinate = tests::published_record(&output, row(&input, 26, "printed-coordinate"));
    let coordinate = &coordinate["observation"]["coordinate"];
    assert_eq!(coordinate["longitude_text"], "105°47′");
    assert_eq!(coordinate["latitude_text"], "26°42′");
    assert_eq!(coordinate["datum"], "unspecified");
    assert_eq!(coordinate["resolution_arcminutes"], 1.0);
}
