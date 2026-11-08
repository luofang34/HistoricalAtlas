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
