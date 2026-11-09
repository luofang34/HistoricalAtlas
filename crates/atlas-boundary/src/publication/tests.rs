use super::*;

#[test]
fn split_geometry_checks_bytes_before_decoding_and_rejects_external_paths() {
    let bytes = serde_json::to_vec(&GeometryBundle::default()).unwrap();
    let parts: Vec<_> = bytes.chunks(7).map(Vec::from).collect();
    let mut reference = GeometryReference {
        bounds: None,
        subjects: vec![],
        parts: parts
            .iter()
            .enumerate()
            .map(|(i, bytes)| GeometryPart {
                path: format!("geometry/example/{i}.part"),
                bytes: bytes.len(),
                sha256: fingerprint(bytes),
            })
            .collect(),
    };
    assert!(reference.decode(&parts).unwrap().boundaries.is_empty());
    let mut damaged = parts.clone();
    damaged[0][0] ^= 1;
    assert!(matches!(
        reference.decode(&damaged),
        Err(GeometryError::Part(_))
    ));
    reference.parts[0].path = "geometry/../../private.part".into();
    assert!(reference.validate().is_err());
}
