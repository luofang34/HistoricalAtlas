use super::*;

#[test]
fn source_names_find_their_places_and_rivers_across_pages() {
    let atlas = atlas();
    for (name, key) in [
        ("青岗林", "lanni/qingganglin"),
        ("青杠林", "lanni/qingganglin"),
        ("倮罗", "guole/guoleqiao"),
        ("倮倮桥", "guole/guoleqiao"),
        ("倮罗桥", "guole/guole-bridge"),
        ("果乐河", "haizijie/qiansuo-river"),
        ("前所河", "haizijie/qiansuo-river"),
        ("豪沟河", "haizijie/haogou-river"),
        ("刘华屯", "liuhua/liuhua-dazhai"),
        ("坡头村", "wangjiaba/potoushang"),
        ("坡头上", "wangjiaba/potoushang"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id), "{key}");
    }
    let id = EntityId::from_key("earth/cn-gz/bijie/liuhua-commune").unwrap();
    let populations: BTreeSet<_> = atlas
        .entry(id)
        .unwrap()
        .records
        .iter()
        .filter_map(|r| r.observation.population)
        .collect();
    assert_eq!(populations, BTreeSet::from([3213, 3302]));
}

#[test]
fn two_daomakan_villages_and_the_recorded_tree_remain_separate() {
    let atlas = atlas();
    let ids: BTreeSet<_> = ["daomakan-seat", "daomakan-east"]
        .map(|key| EntityId::from_key(&format!("earth/cn-gz/bijie/lanni/{key}")).unwrap())
        .into_iter()
        .collect();
    let matches: BTreeSet<_> = atlas.search("倒马坎", BodyId::Earth).into_iter().collect();
    assert!(ids.is_subset(&matches));
    assert_eq!(ids.len(), 2);
    let tree = EntityId::from_key("earth/cn-gz/bijie/wangjiaba/potoushang-ginkgo").unwrap();
    let village = EntityId::from_key("earth/cn-gz/bijie/wangjiaba/potoushang").unwrap();
    assert_eq!(atlas.parents(tree), BTreeSet::from([village]));
}

#[test]
fn xiaoba_and_zhongtun_historical_names_find_their_settlements() {
    let atlas = atlas();
    for (name, key) in [
        ("杨柳街", "xiaoba/xiaoba"),
        ("小坝屯", "xiaoba/xiaoba"),
        ("旱屯", "hantun/hantun"),
        ("阿冲屯", "zhongtun/zhongtun"),
        ("窝中屯", "zhongtun/zhongtun"),
        ("新场河", "xiaoba/xinchang-river"),
        ("水牛屯电站", "zhongtun/shuiniutun-power-station"),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}")).unwrap();
        assert!(atlas.search(name, BodyId::Earth).contains(&id), "{key}");
    }
    let matched: BTreeSet<_> = atlas.search("和尚冲", BodyId::Earth).into_iter().collect();
    for key in ["heshangchong-north", "heshangchong-south"] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/xiaoba/{key}")).unwrap();
        assert!(matched.contains(&id));
    }
}
