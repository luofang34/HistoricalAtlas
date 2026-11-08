use super::*;

#[cfg(test)]
pub(crate) fn publication() -> Publication {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = atlas_compile::input::load_blocking(&root).unwrap();
    let output = atlas_compile::export::compile(&input).unwrap();
    let sources =
        serde_json::from_slice(&output[&std::path::PathBuf::from("sources.json")]).unwrap();
    let entries = output
        .iter()
        .filter(|(path, _)| path.starts_with("entities/index"))
        .flat_map(|(_, bytes)| serde_json::from_slice::<BTreeMap<EntityId, Entry>>(bytes).unwrap())
        .collect();
    Publication { sources, entries }
}

#[cfg(test)]
pub(crate) fn atlas() -> Atlas {
    Atlas::new(publication()).unwrap()
}

#[test]
fn compiled_shards_search_former_names_and_keep_coincident_villages() {
    let atlas = atlas();
    let former = atlas.search("灭资", BodyId::Earth);
    assert_eq!(former.len(), 1);
    assert_eq!(atlas.label(former[0]), "驿陇大队");
    assert!(atlas.search("灭资", BodyId::Moon).is_empty());
    let shaer = atlas
        .search("厦耳", BodyId::Earth)
        .into_iter()
        .find(|id| atlas.label(*id) == "厦耳")
        .unwrap();
    let bengtushan = atlas.search("崩土山", BodyId::Earth)[0];
    assert_ne!(shaer, bengtushan);
    let point = |id| {
        atlas.entry(id).unwrap().records[0]
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .point
    };
    assert_eq!(point(shaer), point(bengtushan));
    assert!(
        atlas
            .parents(shaer)
            .iter()
            .any(|parent| atlas.children(*parent).contains(&shaer))
    );
}

#[test]
fn reject_unreviewed_and_misfiled_evidence() {
    let mut publication = publication();
    let entry = publication
        .entries
        .values_mut()
        .find(|entry| !entry.records.is_empty())
        .unwrap();
    entry.records[0].checked = false;
    assert!(matches!(
        Atlas::new(publication),
        Err(AppError::Unchecked(_))
    ));
    let mut publication = self::publication();
    let entry = publication
        .entries
        .values_mut()
        .find(|entry| !entry.records.is_empty())
        .unwrap();
    entry.records[0].entity_id = EntityId::from_key("earth/another-entity").unwrap();
    assert!(matches!(
        Atlas::new(publication),
        Err(AppError::EvidenceIdentity(_))
    ));
}
