use super::*;

#[path = "haizijie/tests.rs"]
mod haizijie;

#[test]
fn shaogang_local_names_and_shashao_facilities_resolve_across_sources() {
    let atlas = atlas();
    for (name, key) in [
        ("烧缸箐", "shaogang/shaogang"),
        ("烧杠箐", "shaogang/shaogang"),
        ("邵官堡", "shashao/shaoguanpu"),
        ("王张官屯", "wangzhang/wangzhangjie"),
        ("白马山", "shaogang/baimashan"),
        ("白马山", "shaogang/dashan"),
        ("白马山", "shaogang/baima-mountain"),
        ("白马山", "wangzhang/baima-mountain"),
        ("沙邵鱼场", "shashao/fish-farm"),
        ("沙邵渔场", "shashao/fish-farm"),
        ("海子街中学", "haizijie/middle-school"),
        ("白马山林场", "haizijie/baimashan-state-forest"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id), "{key}");
    }
    let school = EntityId::from_key("earth/cn-gz/bijie/haizijie/middle-school").unwrap();
    let brigade = EntityId::from_key("earth/cn-gz/bijie/shashao/shaoguan-brigade").unwrap();
    assert_eq!(atlas.parents(school), BTreeSet::from([brigade]));
}

#[test]
fn tianbaqiao_and_pangxie_source_names_resolve_without_merging_distinct_places() {
    let atlas = atlas();
    for (name, key) in [
        ("阿胖田坝", "tianbaqiao/tianbaqiao"),
        ("大田坝", "tianbaqiao/tianbaqiao"),
        ("疙兜街", "pangxie/pangxiejing"),
        ("螃蟹井街上", "pangxie/pangxiejing"),
        ("臭水井", "pangxie/xiushuijing-brigade"),
        ("嗅水井大队", "pangxie/xiushuijing-brigade"),
        ("硝灰洞", "pangxie/xiaohuidong-brigade"),
        ("哨灰洞大队", "pangxie/xiaohuidong-brigade"),
        ("石㮟㮟", "pangxie/shikala"),
        ("田坝河", "tianbaqiao/tianba-river"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id), "{key}");
    }
    let village = EntityId::from_key("earth/cn-gz/bijie/pangxie/shanbeihou").unwrap();
    let brigade = EntityId::from_key("earth/cn-gz/bijie/pangxie/xiaohuidong-brigade").unwrap();
    assert_eq!(atlas.parents(village), BTreeSet::from([brigade]));
    let village = EntityId::from_key("earth/cn-gz/bijie/tianbaqiao/heijiazhai").unwrap();
    let commune = EntityId::from_key("earth/cn-gz/bijie/tianbaqiao-commune").unwrap();
    assert_eq!(atlas.parents(village), BTreeSet::from([commune]));
}

#[test]
fn haizijie_names_and_river_aliases_resolve_to_their_cited_entities() {
    let atlas = atlas();
    for (name, key) in [
        ("海子铺", "haizijie/haizijie"),
        ("福集公社", "yingbin-commune"),
        ("红卫公社", "guanyinqiao-commune"),
        ("青岗林", "yingbin/qingganglin"),
        ("青杠林", "yingbin/qingganglin"),
        ("威镇河", "guanyinqiao-river"),
        ("田坝桥河", "guanyinqiao-river"),
        ("观音桥", "guanyinqiao/guanyinqiao"),
        ("观音桥", "guanyinqiao/guanyin-bridge"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id), "{key}");
    }
    let commune = EntityId::from_key("earth/cn-gz/bijie/yingbin-commune").unwrap();
    let populations: BTreeSet<_> = atlas
        .entry(commune)
        .unwrap()
        .records
        .iter()
        .filter_map(|r| r.observation.population)
        .collect();
    assert_eq!(populations, BTreeSet::from([3079, 3135]));
}

#[test]
fn heguantun_communes_expose_same_named_places_and_cited_river_aliases() {
    let atlas = atlas();
    for (name, key) in [
        ("白纸厂", "daba/baizhichang"),
        ("大岩洞", "daba/dayandong"),
        ("大岩洞", "daba/dayan-cave"),
        ("杉寨", "shanzhai/shanzhai"),
        ("杉寨", "shanzhai/shanzhai-place"),
        ("背水岩洞", "shanzhai/beishuiyandong"),
        ("背水岩洞", "shanzhai/beishuiyan-cave"),
        ("胡家寨", "houqing/houqing-hujiazhai"),
        ("胡家寨", "houqing/baiguo-hujiazhai"),
        ("响水河", "heguantun-district/houqing-river"),
        ("利民水库", "heguantun-district/limin-reservoir"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id), "{key}");
    }
    let commune = EntityId::from_key("earth/cn-gz/bijie/houqing-commune").unwrap();
    let populations: BTreeSet<_> = atlas
        .entry(commune)
        .unwrap()
        .records
        .iter()
        .filter_map(|record| record.observation.population)
        .collect();
    assert_eq!(populations, BTreeSet::from([4177, 14177]));
}

#[test]
fn heguantun_names_and_shuiqing_unresolved_coordinates_remain_searchable() {
    let atlas = atlas();
    for (name, key) in [
        ("官屯公社", "heguantun-commune"),
        ("北望村", "heguantun/heguantun"),
        ("对坡区", "duipo/former-district"),
        ("Qīngshīkālā", "shuiqing/qingshikala"),
        ("下流水岩", "shuiqing/xialiushuiyan"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id));
    }
    for key in ["shisun", "shisun-rock"] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/heguantun/{key}")).unwrap();
        assert!(atlas.search("石笋", BodyId::Earth).contains(&id));
    }
    let unresolved = EntityId::from_key("earth/cn-gz/bijie/shuiqing/xialiushuiyan").unwrap();
    let entry = atlas.entry(unresolved).unwrap();
    assert!(
        entry
            .records
            .iter()
            .all(|record| record.observation.coordinate.is_none())
    );
    assert!(entry.records.iter().any(|record| {
        record
            .observation
            .unresolved_coordinate
            .as_ref()
            .is_some_and(|coordinate| coordinate.longitude_text == "105°87′")
    }));
}

#[test]
fn wangfeng_huangnichong_and_shaba_names_resolve_to_their_own_evidence() {
    let atlas = atlas();
    for (name, key) in [
        ("王洪屯", "wangfeng/wangfengtun"),
        ("黄泥公社", "huangnichong-commune"),
        ("云山铺", "shaba/gaoshanpu"),
        ("干河沟", "gezuo/gezuo-river"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id));
    }
    let matches = atlas.search("高山铺", BodyId::Earth);
    let mut expected: BTreeSet<_> = ["gaoshanpu", "gaoshanpu-former-commune", "gaoshanpu-daoban"]
        .map(|key| EntityId::from_key(&format!("earth/cn-gz/bijie/shaba/{key}")).unwrap())
        .into_iter()
        .collect();
    expected.insert(EntityId::from_key("earth/cn-gz/bijie/gaoshanpu-commune").unwrap());
    assert_eq!(matches.into_iter().collect::<BTreeSet<_>>(), expected);
    for (commune, count) in [("wangfeng", 8), ("huangnichong", 4), ("shaba", 6)] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{commune}-commune")).unwrap();
        let brigades = atlas
            .children(id)
            .into_iter()
            .filter(|child| {
                atlas.entry(*child).unwrap().entity.kind == atlas_ledger::model::EntityKind::Brigade
            })
            .count();
        assert_eq!(brigades, count);
    }
}

#[test]
fn changchunbao_former_names_and_mine_spellings_resolve_to_cited_identities() {
    let atlas = atlas();
    for (name, key) in [
        ("长春公社", "changchunbao-commune"),
        ("丰乐公社", "qingfeng-commune"),
        ("南加河", "changchunbao/changchun-river"),
        ("垭关煤厂", "changchunbao/yaguan-coal"),
        ("鸦关煤厂", "changchunbao/yaguan-coal"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert_eq!(atlas.search(name, BodyId::Earth), vec![id]);
    }
    let matches = atlas.search("倒洞坡", BodyId::Earth);
    assert_eq!(matches.len(), 2);
    for key in ["daodongpo", "daodongpo-cave"] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/qingfeng/{key}")).unwrap();
        assert!(matches.contains(&id));
        assert!(
            atlas
                .entry(id)
                .unwrap()
                .records
                .iter()
                .all(|record| record.page_or_sheet.as_deref() == Some("108"))
        );
    }
}

#[test]
fn dahe_bridges_and_river_alias_resolve_to_their_distinct_evidence() {
    let atlas = atlas();
    let bridges = atlas.search("大河桥", BodyId::Earth);
    let expected: BTreeSet<_> = ["dahe-bridge-30m", "dahe-bridge-48m"]
        .map(|key| EntityId::from_key(&format!("earth/cn-gz/bijie/dahe/{key}")).unwrap())
        .into_iter()
        .collect();
    assert_eq!(bridges.into_iter().collect::<BTreeSet<_>>(), expected);
    let notes: BTreeSet<_> = expected
        .iter()
        .flat_map(|id| &atlas.entry(*id).unwrap().records)
        .filter_map(|record| record.observation.note.as_deref())
        .collect();
    assert_eq!(
        notes,
        BTreeSet::from(["公路桥 (30.4–5.5)/15", "公路桥 (48.5–5)/15"])
    );
    let river = EntityId::from_key("earth/cn-gz/bijie/dahe/dahe-river").unwrap();
    assert!(atlas.search("红岩河", BodyId::Earth).contains(&river));
    assert!(atlas.search("大河", BodyId::Earth).contains(&river));
}

#[test]
fn qingsong_river_alias_and_coincident_mountain_village_remain_searchable() {
    let atlas = atlas();
    let river = EntityId::from_key("earth/cn-gz/bijie/qingsong/dahe").unwrap();
    assert_eq!(atlas.search("青松河", BodyId::Earth), vec![river]);
    assert!(atlas.search("大河", BodyId::Earth).contains(&river));
    let names = atlas.search("苗营大山", BodyId::Earth);
    for key in ["miaoying-dashan", "miaoying-dashan-village"] {
        assert!(
            names.contains(
                &EntityId::from_key(&format!("earth/cn-gz/bijie/qingsong/{key}")).unwrap()
            )
        );
    }
}

#[test]
fn yachi_commune_renaming_and_source_spellings_remain_searchable() {
    let atlas = atlas();
    let commune = EntityId::from_key("earth/cn-gz/bijie/ganhegou-commune").unwrap();
    assert_eq!(atlas.search("甘河公社", BodyId::Earth), vec![commune]);
    assert!(atlas.search("甘河沟公社", BodyId::Earth).contains(&commune));
    let locality = EntityId::from_key("earth/cn-gz/bijie/caodi/caodi-chong").unwrap();
    assert_eq!(atlas.search("炒铁冲", BodyId::Earth), vec![locality]);
    let river = EntityId::from_key("earth/cn-gz/bijie/songlin/kuaqiao-he").unwrap();
    assert_eq!(atlas.search("跨桥河", BodyId::Earth), vec![river]);
    assert!(atlas.search("垮桥河", BodyId::Earth).contains(&river));
}

#[test]
fn yachi_street_history_and_cross_page_villages_are_searchable() {
    let atlas = atlas();
    let street = EntityId::from_key("earth/cn-gz/bijie/yachi/yachi-jie").unwrap();
    assert_eq!(atlas.search("鸭子塘街", BodyId::Earth), vec![street]);
    let village = EntityId::from_key("earth/cn-gz/bijie/nanqing/liujiadong").unwrap();
    let brigade = EntityId::from_key("earth/cn-gz/bijie/nanqing/hetao-brigade").unwrap();
    assert!(atlas.search("刘家洞", BodyId::Earth).contains(&village));
    assert_eq!(atlas.parents(village), BTreeSet::from([brigade]));
    let communes = atlas.children(EntityId::from_key("earth/cn-gz/bijie/yachi-district").unwrap());
    assert_eq!(communes.len(), 10);
}

#[test]
fn urban_search_keeps_same_named_temples_distinct_and_follows_office_hierarchy() {
    let atlas = atlas();
    let matches = atlas.search("财神庙", BodyId::Earth);
    assert_eq!(matches.len(), 2);
    let temple_notes: BTreeSet<_> = matches
        .iter()
        .flat_map(|id| &atlas.entry(*id).unwrap().records)
        .filter_map(|record| record.observation.note.clone())
        .collect();
    assert_eq!(
        temple_notes,
        BTreeSet::from([
            "已废，改居民住房。".into(),
            "已废，民国时设商会，现为人民剧场。".into()
        ])
    );
    let road = EntityId::from_key("earth/cn-gz/bijie/urban/guihua-road").unwrap();
    let office = EntityId::from_key("earth/cn-gz/bijie/urban/weining-road-office").unwrap();
    let town = EntityId::from_key("earth/cn-gz/bijie/bijie-town").unwrap();
    assert_eq!(atlas.parents(road), BTreeSet::from([office]));
    assert_eq!(atlas.parents(office), BTreeSet::from([town]));
    assert!(atlas.children(office).contains(&road));
}

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
    let aliases =
        serde_json::from_slice(&output[&std::path::PathBuf::from("entities/aliases.json")])
            .unwrap();
    Publication {
        sources,
        entries,
        aliases,
    }
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
fn reject_aliases_that_do_not_resolve_to_published_evidence() {
    let old = EntityId::from_key("earth/old-id").unwrap();
    let missing = EntityId::from_key("earth/missing-id").unwrap();
    for destination in [old, missing] {
        let mut publication = publication();
        publication.aliases.0.insert(old, destination);
        assert!(matches!(
            Atlas::new(publication),
            Err(AppError::Validation(_))
        ));
    }
}

#[test]
fn bijie_county_browses_the_current_index_and_the_cited_former_district() {
    let atlas = atlas();
    let county = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    let districts = atlas.children(county);
    assert_eq!(districts.len(), 16);
    let former = EntityId::from_key("earth/cn-gz/bijie/duipo/former-district").unwrap();
    assert!(districts.contains(&former));
    assert!(atlas.children(former).is_empty());
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
        let children: Vec<_> = atlas
            .children(id)
            .into_iter()
            .filter(|child| {
                atlas.entry(*child).unwrap().entity.kind == atlas_ledger::model::EntityKind::Commune
            })
            .collect();
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
