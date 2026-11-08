use super::*;
use serde_json::{Value, json};
use std::path::Path;

#[test]
fn byte_bounded_chunks_round_trip_unicode_items_and_geojson_features() {
    let values: Vec<_> = (0..40)
        .map(|index| json!({"id": index, "name": "五里坪公社"}))
        .collect();
    for kind in [ChunkKind::Array, ChunkKind::GeoJson] {
        let mut output = Artifacts::new();
        let paths = partition(&mut output, "territory/test", &values, kind, 250).unwrap();
        assert!(paths.len() > 2);
        let mut restored = Vec::new();
        for path in paths {
            let bytes = &output[Path::new(&path)];
            assert!(bytes.len() < 250);
            let mut value: Value = serde_json::from_slice(bytes).unwrap();
            let array = match kind {
                ChunkKind::Array => value.as_array_mut().unwrap(),
                ChunkKind::GeoJson => value["features"].as_array_mut().unwrap(),
            };
            assert!(!array.is_empty());
            restored.append(array);
        }
        assert_eq!(restored, values);
    }
}

#[test]
fn exact_budget_and_indivisible_items_are_rejected_without_a_partial_artifact() {
    let mut output = Artifacts::new();
    let values = ["a"];
    assert!(partition(&mut output, "territory/test", &values, ChunkKind::Array, 5).is_err());
    assert!(output.is_empty());
    let paths = partition(&mut output, "territory/test", &values, ChunkKind::Array, 6).unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(output[Path::new(&paths[0])], b"[\"a\"]");
    assert!(
        partition::<Value>(&mut output, "territory/empty", &[], ChunkKind::Array, 6)
            .unwrap()
            .is_empty()
    );
}
