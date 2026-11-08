use super::*;

#[test]
fn load_compiler_output_and_reject_manifest_path_traversal() {
    let input = atlas_compile::input::load_blocking(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
    )
    .unwrap();
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
    fs::write(output.join("entities/shards.json"), b"[\"../../outside\"]").unwrap();
    assert!(matches!(
        load_blocking(&output),
        Err(PublicationError::Shard(_))
    ));
}
