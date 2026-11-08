use super::*;
use crate::input::load_blocking;

#[test]
fn real_ledger_builds_deterministically_and_omits_unchecked_rows() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    let output = compile(&input).unwrap();
    assert_eq!(output, compile(&input).unwrap());
    assert!(output.values().all(|bytes| bytes.len() < MAX_FILE_BYTES));
    let record = input
        .data
        .records
        .iter_mut()
        .find(|r| r.observation.name == "小冲")
        .unwrap();
    let id = record.entity_id;
    record.checked = false;
    let filtered = compile(&input).unwrap();
    let index = PathBuf::from(format!("entities/index/{}.json", id.shard()));
    if let Some(bytes) = filtered.get(&index) {
        let json: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert!(json.get(id.0.to_string()).is_none());
    }
    input.published.0.insert(id);
    assert!(compile(&input).is_err());
}

#[test]
fn strict_file_budget_rejects_oversize_output() {
    let mut artifacts = Artifacts::new();
    let value = "x".repeat(MAX_FILE_BYTES);
    assert!(matches!(
        insert(&mut artifacts, "large.json", &value),
        Err(ExportError::Oversize { .. })
    ));
    assert!(artifacts.is_empty());
}

#[test]
fn shared_minute_coordinates_preserve_distinct_villages_and_their_parent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let village = |name: &str| {
        input
            .data
            .records
            .iter()
            .find(|record| {
                record.source_id == "anshun-county-gazetteer" && record.observation.name == name
            })
            .unwrap()
    };
    let shaer = village("厦耳");
    let bengtushan = village("崩土山");
    let brigade = village("厦耳大队");
    assert_ne!(shaer.entity_id, bengtushan.entity_id);
    assert_eq!(shaer.observation.parent_id, Some(brigade.entity_id));
    assert_eq!(bengtushan.observation.parent_id, Some(brigade.entity_id));
    let output = compile(&input).unwrap();
    for record in [shaer, bengtushan] {
        let path = PathBuf::from(format!("entities/index/{}.json", record.entity_id.shard()));
        let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
        let published = &shard[record.entity_id.0.to_string()]["records"][0];
        assert_eq!(published["scan_page"], 19);
        assert_eq!(published["page_or_sheet"], "12");
        let coordinate = &published["observation"]["coordinate"];
        assert_eq!(coordinate["longitude_text"], "105°56′");
        assert_eq!(coordinate["latitude_text"], "26°23′");
        assert_eq!(coordinate["resolution_arcminutes"], 1.0);
        assert_eq!(coordinate["datum"], "unspecified");
    }
}

#[test]
fn temporal_hierarchy_cannot_publish_a_parent_without_checked_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    let unchecked = input
        .data
        .records
        .iter()
        .find(|record| !record.checked)
        .unwrap()
        .entity_id;
    let estimate = input
        .data
        .estimates
        .iter_mut()
        .find(|estimate| {
            matches!(
                &estimate.value, EstimateValue::Temporal { assertion }
                    if matches!(assertion.value, TemporalValue::Hierarchy(_))
            )
        })
        .unwrap();
    if let EstimateValue::Temporal { assertion } = &mut estimate.value
        && let TemporalValue::Hierarchy(hierarchy) = &mut assertion.value
    {
        hierarchy.parent_id = Some(unchecked);
    }
    assert!(matches!(compile(&input), Err(ExportError::MissingEvidence(id)) if id == unchecked));
}

#[cfg(test)]
fn bijie_record<'a>(input: &'a LedgerInput, key: &str) -> &'a Record {
    input
        .data
        .records
        .iter()
        .find(|record| record.key == key)
        .unwrap()
}

#[cfg(test)]
fn published_record(output: &Artifacts, record: &Record) -> serde_json::Value {
    let path = PathBuf::from(format!("entities/index/{}.json", record.entity_id.shard()));
    let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
    shard[record.entity_id.0.to_string()]["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["id"] == record.id.0.to_string())
        .unwrap()
        .clone()
}

#[test]
fn bijie_export_keeps_printed_coordinate_anomalies_and_blank_cells() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let output = compile(&input).unwrap();
    for (key, latitude, longitude) in [
        ("bijie-gazetteer/p24/daqiao/zhongba", "27°28′", "105°16′"),
        ("bijie-gazetteer/p28/dalan/wujia-gou", "20°16′", "105°16′"),
        ("bijie-gazetteer/p38/toubu/yakou", "24°17′", "105°19′"),
        (
            "bijie-gazetteer/p38/toubu/yingjiao-brigade",
            "27°16′",
            "106°21′",
        ),
        (
            "bijie-gazetteer/p31/dehou/lujia-qiao-bridge",
            "27°18′",
            "102°18′",
        ),
    ] {
        let record = bijie_record(&input, key);
        let published = published_record(&output, record);
        let coordinate = &published["observation"]["coordinate"];
        assert_eq!(coordinate["latitude_text"], latitude);
        assert_eq!(coordinate["longitude_text"], longitude);
        assert_eq!(coordinate["datum"], "unspecified");
        assert_eq!(coordinate["resolution_arcminutes"], 1.0);
    }
    let blank = bijie_record(&input, "bijie-gazetteer/p24/daqiao/huangjia-dayuan");
    let published = published_record(&output, blank);
    assert!(published["observation"]["population"].is_null());
    assert!(
        published["observation"]["ethnicities"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let fifteen = bijie_record(&input, "bijie-gazetteer/p24/daqiao/poping-zi");
    assert_eq!(
        published_record(&output, fifteen)["observation"]["population"],
        15
    );
}

#[test]
fn explicit_brigade_notes_and_continued_tables_link_to_the_correct_parent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let output = compile(&input).unwrap();
    let daqiao = EntityId::from_key("earth/cn-gz/bijie/daqiao/daqiao-brigade").unwrap();
    for slug in ["shuiguan-suo", "taoci-chang", "television-relay"] {
        let record = bijie_record(&input, &format!("bijie-gazetteer/p24/daqiao/{slug}"));
        assert_eq!(
            published_record(&output, record)["observation"]["parent_id"],
            daqiao.0.to_string()
        );
    }
    let tianshengqiao =
        EntityId::from_key("earth/cn-gz/bijie/daqiao/tianshengqiao-brigade").unwrap();
    for slug in [
        "xuhua-tun",
        "maomao-chong",
        "zhangji-tun",
        "shuangshan",
        "long-daishi",
        "zhujia-shu",
    ] {
        let record = bijie_record(&input, &format!("bijie-gazetteer/p25/daqiao/{slug}"));
        assert_eq!(
            published_record(&output, record)["observation"]["parent_id"],
            tianshengqiao.0.to_string()
        );
    }
    let commune = bijie_record(&input, "bijie-gazetteer/p28/dalan/dalan-commune");
    let seat = bijie_record(&input, "bijie-gazetteer/p28/dalan/maojia-yakou");
    assert_eq!(
        published_record(&output, commune)["observation"]["seat_id"],
        seat.entity_id.0.to_string()
    );
}

#[test]
fn shared_names_and_coordinates_do_not_merge_distinct_geographical_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let output = compile(&input).unwrap();
    for (first, second) in [
        (
            "bijie-gazetteer/p24/daqiao/xiaohe-seat",
            "bijie-gazetteer/p24/daqiao/xiaohe-village",
        ),
        (
            "bijie-gazetteer/p28/dalan/maoer-dong-village",
            "bijie-gazetteer/p28/dalan/maoer-dong-cave",
        ),
        (
            "bijie-gazetteer/p21/cuiping/chating-village",
            "bijie-gazetteer/p21/cuiping/chating-pavilion",
        ),
    ] {
        let first = bijie_record(&input, first);
        let second = bijie_record(&input, second);
        assert_ne!(first.entity_id, second.entity_id);
        assert_eq!(first.observation.name, second.observation.name);
        assert_eq!(
            first.observation.coordinate.as_ref().unwrap().point,
            second.observation.coordinate.as_ref().unwrap().point
        );
        assert_eq!(
            published_record(&output, first)["id"],
            first.id.0.to_string()
        );
        assert_eq!(
            published_record(&output, second)["id"],
            second.id.0.to_string()
        );
    }
    let river = EntityId::from_key("earth/cn-gz/bijie/daotianhe-river").unwrap();
    let records: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.entity_id == river)
        .collect();
    assert_eq!(records.len(), 4);
    assert!(
        records
            .iter()
            .all(|record| record.observation.coordinate.is_none())
    );
    assert!(
        records
            .iter()
            .all(|record| published_record(&output, record)["observation"]["coordinate"].is_null())
    );
}

#[test]
fn toubu_and_wuliping_tables_preserve_seats_continuations_and_source_disagreements() {
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let output = compile(&input).unwrap();
    let continued = bijie_record(&input, "bijie-gazetteer/p39/toubu/xindianzi");
    let brigade = bijie_record(&input, "bijie-gazetteer/p38/toubu/yingjiao-brigade");
    assert_eq!(continued.observation.parent_id, Some(brigade.entity_id));
    assert_eq!(continued.observation.population, Some(105));
    for (overview, table, first, second) in [
        (
            "p37/toubu/composition",
            "p38/toubu/toubu-commune",
            9732,
            9891,
        ),
        (
            "p33/wuliping/composition",
            "p35/wuliping/wuliping-commune",
            3046,
            3046,
        ),
    ] {
        for (key, expected) in [(overview, first), (table, second)] {
            let record = bijie_record(&input, &format!("bijie-gazetteer/{key}"));
            assert_eq!(
                published_record(&output, record)["observation"]["population"],
                expected
            );
        }
    }
    for (prefix, place, landmark) in [
        (
            "wuliping",
            "p35/wuliping/shiban-jing-village",
            "p35/wuliping/shiban-jing-well",
        ),
        (
            "toubu",
            "p38/toubu/toubu-qiao-village",
            "p39/toubu/toubu-qiao-bridge",
        ),
        (
            "toubu",
            "p38/toubu/liucang-qiao-village",
            "p39/toubu/liucang-qiao-bridge",
        ),
    ] {
        let village = bijie_record(&input, &format!("bijie-gazetteer/{place}"));
        let landmark = bijie_record(&input, &format!("bijie-gazetteer/{landmark}"));
        assert_ne!(village.entity_id, landmark.entity_id, "{prefix}");
        assert_eq!(village.observation.name, landmark.observation.name);
        for record in [village, landmark] {
            assert_eq!(
                published_record(&output, record)["id"],
                record.id.0.to_string()
            );
        }
    }
    let commune = bijie_record(&input, "bijie-gazetteer/p35/wuliping/wuliping-commune");
    let seat = bijie_record(&input, "bijie-gazetteer/p35/wuliping/miaopu");
    assert_eq!(commune.observation.seat_id, Some(seat.entity_id));
    assert!(published_record(&output, seat)["observation"]["population"].is_null());
    let overview = bijie_record(
        &input,
        "bijie-gazetteer/p33/wuliping/zhaishang-population-group",
    );
    let table = bijie_record(&input, "bijie-gazetteer/p35/wuliping/zhaishang");
    assert_eq!(overview.entity_id, table.entity_id);
    assert_eq!(overview.observation.ethnicities, ["苗"]);
    assert_eq!(table.observation.ethnicities, ["汉"]);
    assert_eq!(table.observation.population, Some(273));
    assert!(overview.verbatim_quote.as_ref().unwrap().contains("473"));
}

#[test]
fn conflicting_source_totals_and_undated_parent_claims_survive_export() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let output = compile(&input).unwrap();
    let overview = bijie_record(&input, "bijie-gazetteer/p29/dehou/composition");
    let table = bijie_record(&input, "bijie-gazetteer/p30/dehou/dehou-commune");
    assert_eq!(overview.entity_id, table.entity_id);
    assert_eq!(
        published_record(&output, overview)["observation"]["population"],
        2882
    );
    assert_eq!(
        published_record(&output, table)["observation"]["population"],
        7882
    );
    let overview = bijie_record(&input, "bijie-gazetteer/p27/chating/parent");
    let table = bijie_record(&input, "bijie-gazetteer/p21/cuiping/chating-brigade");
    assert_eq!(overview.entity_id, table.entity_id);
    assert_ne!(overview.observation.parent_id, table.observation.parent_id);
    for record in [overview, table] {
        let published = published_record(&output, record);
        assert!(published["source_date"].is_null());
        assert_eq!(
            published["scan_page"].as_u64(),
            record.scan_page.map(u64::from)
        );
        assert_eq!(
            published["observation"]["parent_id"],
            record.observation.parent_id.unwrap().0.to_string()
        );
    }
}

#[test]
fn contents_citations_remain_distinct_from_main_text_pages_and_dates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let output = compile(&input).unwrap();
    let contents: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.key.starts_with("bijie-gazetteer/contents-"))
        .collect();
    assert_eq!(contents.len(), 139);
    for record in contents {
        let published = published_record(&output, record);
        let scan = published["scan_page"].as_u64().unwrap();
        assert!((30..=35).contains(&scan));
        assert_eq!(
            published["page_or_sheet"],
            format!("Contents {}", scan - 29)
        );
        assert!(published["source_date"].is_null());
        assert!(published["observation"]["coordinate"].is_null());
    }
    let dehou = bijie_record(&input, "bijie-gazetteer/contents-1/dehou-commune");
    let overview = bijie_record(&input, "bijie-gazetteer/p29/dehou/composition");
    assert_eq!(dehou.entity_id, overview.entity_id);
    assert!(
        published_record(&output, dehou)["locator"]
            .as_str()
            .unwrap()
            .ends_with("printed page 28")
    );
    assert_eq!(published_record(&output, overview)["page_or_sheet"], "29");
    let census = bijie_record(&input, "bijie-gazetteer/foreword-3/survey-coverage");
    assert_eq!(published_record(&output, census)["scan_page"], 6);
    assert_eq!(
        published_record(&output, census)["page_or_sheet"],
        "Foreword 3"
    );
    assert_eq!(
        published_record(&output, census)["source_date"]["text"],
        "自1981年4月起，到1982年12月止"
    );
}
