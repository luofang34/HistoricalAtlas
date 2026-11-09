use super::*;
use crate::input::load_blocking;

#[cfg(test)]
fn input() -> LedgerInput {
    load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

#[cfg(test)]
fn id(slug: &str) -> EntityId {
    EntityId::from_key(&format!("earth/cn-gz/zhijin/alie/{slug}")).unwrap()
}

#[cfg(test)]
fn row<'a>(input: &'a LedgerInput, page: u32, slug: &str) -> &'a Record {
    let key = format!("zhijin-gazetteer-scan-b/p{page}/alie/{slug}");
    input.data.records.iter().find(|r| r.key == key).unwrap()
}

#[test]
fn alie_tables_publish_brigades_and_keep_aggregate_population_separate() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    for (brigade, page, count, sum, heading) in [
        ("alie", 38, 8, 1673, 1654),
        ("jiaozi", 38, 14, 1756, 1540),
        ("buzhai", 38, 10, 1269, 1321),
        ("tanglishu", 39, 7, 887, 895),
        ("changpo", 39, 5, 831, 845),
        ("dagou", 39, 5, 949, 974),
    ] {
        let parent = id(&format!("{brigade}-brigade"));
        let rows: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|r| {
                matches!(r.scan_page, Some(46 | 47)) && r.observation.parent_id == Some(parent)
            })
            .collect();
        assert_eq!(rows.len(), count);
        assert_eq!(
            rows.iter()
                .filter_map(|r| r.observation.population)
                .sum::<u64>(),
            sum
        );
        let header = row(&input, page, &format!("{brigade}-brigade"));
        assert_eq!(header.observation.population, Some(heading));
        for record in rows {
            let published = tests::published_record(&output, record);
            assert_eq!(published["observation"]["parent_id"], parent.0.to_string());
            assert!(published["observation"]["coordinate"].is_null());
        }
    }
    let locality = row(&input, 38, "maoli-yuan");
    assert!(locality.observation.rank.is_none());
    assert_eq!(locality.observation.population, Some(277));
    assert!(locality.verbatim_quote.as_ref().unwrap().contains("小洞头"));
    let overview = tests::published_record(&output, row(&input, 37, "overview"));
    assert_eq!(overview["observation"]["population"], 7229);
    assert_eq!(overview["observation"]["households"], 1371);
    assert_eq!(overview["source_date"]["text"], "一九八一年九月十五日");
}

#[test]
fn alie_map_preserves_the_table_seat_disagreement_and_additional_settlement() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    let map = row(&input, 36, "tanglishu-brigade");
    let table = row(&input, 39, "tanglishu-brigade");
    assert_eq!(map.entity_id, table.entity_id);
    assert_eq!(map.observation.seat_id, Some(id("yakou-zhai")));
    assert_eq!(table.observation.seat_id, Some(id("tangli-shu")));
    let trace = input.traces.iter().find(|t| t.scan_page == 44).unwrap();
    assert_eq!(trace.places.len(), 56);
    assert_eq!(trace.units[0].record_id, row(&input, 36, "commune").id);
    let settlement = row(&input, 36, "xiaolan-ba");
    assert!(trace.places.iter().any(|p| p.record_id == settlement.id));
    assert!(settlement.observation.population.is_none());
    assert!(
        !input.data.records.iter().any(|r| {
            r.entity_id == settlement.entity_id && matches!(r.scan_page, Some(46 | 47))
        })
    );
    let coordinate = tests::published_record(&output, row(&input, 37, "printed-coordinate"));
    let coordinate = &coordinate["observation"]["coordinate"];
    assert_eq!(coordinate["longitude_text"], "105°43′");
    assert_eq!(coordinate["latitude_text"], "26°42′");
    assert_eq!(coordinate["datum"], "unspecified");
}
