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

fn sample_shard() -> ShardSummary {
    use atlas_core::time::EpochTime;
    use atlas_geo::body::BodyId;
    let shard_bytes = br#"{"x":1}"#.to_vec();
    ShardSummary {
        id: "earth/detail/z0_x0_y0/00".into(),
        part: VerifiedPart::from_bytes(
            "entities/index/earth/detail/z0_x0_y0/00.json",
            "entities/index/",
            &shard_bytes,
        )
        .unwrap(),
        body: BodyId::Earth,
        overview: false,
        bounds: Some([100.0, 20.0, 105.0, 25.0]),
        temporal: ShardTemporalSummary {
            has_temporal: true,
            open_start: false,
            earliest: Some(EpochTime::from_ticks(10)),
            latest: Some(EpochTime::from_ticks(20)),
            open_end: false,
            plates: Vec::new(),
        },
        depends_on: Vec::new(),
        entity_count: 1,
    }
}

#[test]
fn shard_temporal_summary_prunes_outside_intervals() {
    use atlas_core::time::EpochTime;
    use std::collections::BTreeMap;

    let shard = sample_shard();
    shard.validate().unwrap();
    let empty = BTreeMap::new();
    assert!(
        !shard
            .temporal
            .matches_selected_time(EpochTime::from_ticks(9), &empty)
    );
    assert!(
        shard
            .temporal
            .matches_selected_time(EpochTime::from_ticks(15), &empty)
    );
    assert!(
        !shard
            .temporal
            .matches_selected_time(EpochTime::from_ticks(21), &empty)
    );
}

#[test]
fn publication_manifest_validates_version_paths_and_hashes() {
    use atlas_geo::body::BodyId;

    let sources = br#"[{"id":"s1"}]"#.to_vec();
    let aliases = br#"{}"#.to_vec();
    let id = EntityId::from_key("earth/test/place").unwrap();
    let published = serde_json::to_vec(&vec![id]).unwrap();
    let shard = sample_shard();
    let shard_inv = serde_json::to_vec(&vec![shard]).unwrap();
    let catalog = serde_json::to_vec(&vec![CatalogItem {
        id,
        key: "earth/test/place".into(),
        body: BodyId::Earth,
        kind: atlas_core::taxonomy::EntityKind::Place(atlas_core::taxonomy::PlaceKind::Settlement),
        shard: "earth/detail/z0_x0_y0/00".into(),
        label: "Place".into(),
        terms: Vec::new(),
        ranks: Vec::new(),
        parents: Vec::new(),
    }])
    .unwrap();
    let part = |path: &str, prefix: &str, bytes: &[u8]| {
        VerifiedPart::from_bytes(path, prefix, bytes).unwrap()
    };
    let mut manifest = PublicationManifest {
        schema_version: PUBLICATION_SCHEMA_VERSION,
        sources: vec![part("sources/000.json", "sources/", &sources)],
        aliases: vec![part(
            "entities/aliases/000.json",
            "entities/aliases/",
            &aliases,
        )],
        published: vec![part(
            "entities/published/000.json",
            "entities/published/",
            &published,
        )],
        shard_inventories: vec![part(
            "entities/shards/000.json",
            "entities/shards/",
            &shard_inv,
        )],
        catalog_shards: vec![CatalogShardSummary {
            prefix: "all".into(),
            part: part("entities/catalog/all.json", "entities/catalog/", &catalog),
            count: 1,
        }],
        plate_models: Vec::new(),
    };
    manifest.validate().unwrap();
    assert_eq!(
        decode_vec_parts::<EntityId>(&manifest.published, &[published]).unwrap(),
        vec![id]
    );
    assert!(
        decode_map_parts::<EntityId, EntityId>(&manifest.aliases, &[aliases])
            .unwrap()
            .is_empty()
    );
    manifest.schema_version = 99;
    assert!(matches!(
        manifest.validate(),
        Err(ManifestError::UnsupportedVersion { found: 99, .. })
    ));
}
