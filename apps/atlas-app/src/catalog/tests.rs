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

#[test]
fn bijie_county_browses_all_districts_and_communes_from_compiled_evidence() {
    let atlas = atlas();
    let county = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    let districts = atlas.children(county);
    assert_eq!(districts.len(), 15);
    let mut communes = BTreeSet::new();
    for (slug, count) in [
        ("bijie-town", 6),
        ("yachi-district", 10),
        ("qingchang-district", 6),
        ("changchunbao-district", 6),
        ("heguantun-district", 6),
        ("haizijie-district", 22),
        ("yanzikou-district", 6),
        ("puyi-district", 10),
        ("jichang-district", 6),
        ("qingshuipu-district", 5),
        ("zhuchang-district", 9),
        ("yangjiawan-district", 7),
        ("yindi-district", 7),
        ("linkou-district", 7),
        ("longchangying-district", 11),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{slug}")).unwrap();
        assert!(districts.contains(&id), "{slug}");
        assert_eq!(atlas.parents(id), BTreeSet::from([county]), "{slug}");
        let children = atlas.children(id);
        assert_eq!(children.len(), count, "{slug}");
        for child in children {
            assert!(
                communes.insert(child),
                "commune assigned to multiple districts"
            );
            assert_eq!(atlas.parents(child), BTreeSet::from([id]));
        }
    }
    assert_eq!(communes.len(), 124);
    for (name, slug) in [
        ("沔鱼河公社", "mianyuhe"),
        ("戛拉公社", "gala"),
        ("烙烘公社", "laohong"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{slug}-commune")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id));
    }
}

#[test]
fn bijie_village_can_follow_source_parents_to_the_county() {
    let atlas = atlas();
    let path = [
        "daqiao/xuhua-tun",
        "daqiao/tianshengqiao-brigade",
        "daqiao-commune",
        "bijie-town",
        "bijie-county",
    ];
    for pair in path.windows(2) {
        let child = EntityId::from_key(&format!("earth/cn-gz/bijie/{}", pair[0])).unwrap();
        let parent = EntityId::from_key(&format!("earth/cn-gz/bijie/{}", pair[1])).unwrap();
        assert!(atlas.parents(child).contains(&parent));
        assert!(atlas.children(parent).contains(&child));
    }
}
