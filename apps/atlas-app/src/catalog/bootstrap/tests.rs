use std::path::Path;

use atlas_core::{assertion::AdministrativeRank, identity::EntityId};
use atlas_geo::body::BodyId;

#[test]
fn bootstrap_searches_labels_and_hierarchy_before_streaming_shards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = atlas_compile::input::load_blocking(&root).unwrap();
    input.derivations.clear();
    let artifacts = atlas_compile::export::compile(&input).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("pub");
    atlas_compile::export::write_blocking(&dir, &artifacts).unwrap();

    let bootstrap = crate::publication::load_bootstrap_blocking(&dir).unwrap();
    let mut atlas = crate::catalog::Atlas::from_bootstrap(bootstrap).unwrap();

    let county = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    let village = EntityId::from_key("earth/cn-gz/bijie/daqiao/xuhua-tun").unwrap();
    assert!(atlas.entry(village).is_none());
    assert!(atlas.unloaded_entity_shard(village).is_some());
    assert!(atlas.search("毕节县", BodyId::Earth).contains(&county));
    assert_eq!(atlas.label(county), "毕节县");
    assert_eq!(atlas.children(county).len(), 16);
    assert!(atlas.has_rank(
        EntityId::from_key("earth/cn-gz/bijie/daqiao-commune").unwrap(),
        AdministrativeRank::Commune,
    ));

    let ordered: Vec<_> = atlas
        .ordered_shards()
        .unwrap()
        .into_iter()
        .cloned()
        .collect();
    for shard in &ordered {
        let bytes = std::fs::read(dir.join(&shard.part.path)).unwrap();
        atlas.attach_shard(&shard.id, &bytes).unwrap();
    }
    assert!(atlas.entry(village).is_some());
    assert!(atlas.unloaded_entity_shard(village).is_none());
}
