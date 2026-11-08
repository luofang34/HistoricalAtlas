use super::*;

#[test]
fn namespace_and_key_determine_identity() {
    let key = "earth/cn-gz/bijie/cuiping-commune";
    let entity = EntityId::from_key(key).unwrap();
    assert_eq!(entity, EntityId::from_key(key).unwrap());
    assert_ne!(entity.0, FactId::from_key(key).unwrap().0);
    assert_eq!(entity.0.get_version_num(), 5);
    assert_eq!(entity.shard().len(), 2);
}

#[test]
fn keys_reject_ambiguous_spellings() {
    for key in [
        "",
        "/earth",
        "earth/",
        "earth//moon",
        "Earth",
        "earth/../moon",
        "地球",
    ] {
        assert!(EntityId::from_key(key).is_err(), "{key}");
    }
}
