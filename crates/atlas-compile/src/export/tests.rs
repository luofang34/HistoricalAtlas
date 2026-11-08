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
