use super::*;

#[test]
fn slice_command_writes_shards_and_checked_evidence_with_original_dates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("slice");
    let at = atlas_core::calendar::CivilDate {
        year: 1966,
        month: 7,
        day: 1,
    }
    .to_epoch(atlas_core::calendar::Calendar::Gregorian, 0)
    .unwrap();
    let count = write_slice_blocking(&root, &at.ticks().to_string(), "nominal", &output).unwrap();
    assert!(count > 2);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("slice.json")).unwrap()).unwrap();
    assert_eq!(manifest["at"], at.ticks().to_string());
    assert_eq!(manifest["selection"], "nominal");
    let shard_name = manifest["shards"][0].as_str().unwrap();
    let shard: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output.join(format!("slice/index/{shard_name}.json"))).unwrap(),
    )
    .unwrap();
    let subject = "4b13e508-16cc-53f4-a503-9d03642b966f";
    let properties = shard[subject]["properties"].as_array().unwrap();
    let name = properties
        .iter()
        .find(|property| property["property"]["property"] == "name")
        .unwrap();
    assert_eq!(
        name["candidates"][0]["assertion"]["value"]["value"]["text"],
        "城关镇"
    );
    assert_eq!(name["candidates"][0]["membership"], "certain");
    let evidence: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output.join(format!("entities/index/{shard_name}.json"))).unwrap(),
    )
    .unwrap();
    let record = evidence[subject]["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["source_date"]["text"] == "1962年")
        .unwrap();
    assert_eq!(record["scan_page"], 47);
    assert_eq!(record["page_or_sheet"], "12");
    let disputes = evidence[subject]["disputes"].as_array().unwrap();
    assert_eq!(disputes.len(), 2);
    for dispute in disputes {
        let records = evidence[subject]["records"].as_array().unwrap();
        let mut pages: Vec<_> = dispute["alternatives"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| {
                records.iter().find(|record| record["id"] == *id).unwrap()["scan_page"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        pages.sort();
        assert_eq!(pages, [42, 47]);
    }
    assert!(write_slice_blocking(&root, &at.ticks().to_string(), "nominal", &output).is_err());
}

#[test]
fn query_errors_precede_output_and_repeated_compilation_is_identical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("slice");
    assert!(matches!(
        write_slice_blocking(&root, "1.5", "nominal", &output),
        Err(SliceError::Ticks { .. })
    ));
    assert!(matches!(
        write_slice_blocking(&root, "0", "guess", &output),
        Err(SliceError::Selection(_))
    ));
    assert!(!output.exists());
    let input = load_blocking(&root).unwrap();
    let artifacts =
        compile_slice(&input, EpochTime::from_ticks(0), TimeSelection::Possible).unwrap();
    assert_eq!(
        artifacts,
        compile_slice(&input, EpochTime::from_ticks(0), TimeSelection::Possible).unwrap()
    );
    assert!(
        artifacts
            .values()
            .all(|bytes| bytes.len() < crate::export::MAX_FILE_BYTES)
    );
}
