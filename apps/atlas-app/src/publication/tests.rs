use super::*;

#[test]
fn load_compiler_output_and_reject_schema_traversal_and_checksum_tampering() {
    let mut input = atlas_compile::input::load_blocking(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
    )
    .unwrap();
    input.derivations.clear();
    let artifacts = atlas_compile::export::compile(&input).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let output = temporary.path().join("atlas");
    atlas_compile::export::write_blocking(&output, &artifacts).unwrap();
    let atlas = load_blocking(&output).unwrap();
    assert_eq!(
        atlas.data.records.len(),
        input
            .data
            .records
            .iter()
            .filter(|record| record.checked)
            .count()
    );

    let manifest_bytes = fs::read(output.join("manifest.json")).unwrap();
    let mut wrong_version: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    wrong_version["schema_version"] = serde_json::json!(2);
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec(&wrong_version).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        load_blocking(&output),
        Err(PublicationError::Manifest(
            ManifestError::UnsupportedVersion {
                found: 2,
                supported: 1
            }
        ))
    ));

    let mut bad_path: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    bad_path["sources"][0]["path"] = serde_json::json!("../../outside.json");
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec(&bad_path).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        load_blocking(&output),
        Err(PublicationError::Manifest(ManifestError::Invalid(_)))
    ));

    fs::write(output.join("manifest.json"), &manifest_bytes).unwrap();
    fs::write(output.join("sources/000.json"), b"[]").unwrap();
    assert!(matches!(
        load_blocking(&output),
        Err(PublicationError::Manifest(_))
    ));
}
