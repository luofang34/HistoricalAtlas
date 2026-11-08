use super::*;
use crate::model::{Entity, EntityKind};
use atlas_geo::body::BodyId;

#[test]
fn published_links_require_a_live_entity_or_resolving_alias() {
    let old = EntityId::from_key("earth/old").unwrap();
    let middle = EntityId::from_key("earth/middle").unwrap();
    let current = EntityId::from_key("earth/current").unwrap();
    let dataset = Dataset {
        entities: vec![Entity {
            id: current,
            key: "earth/current".into(),
            body: BodyId::Earth,
            kind: EntityKind::Town,
        }],
        ..Dataset::default()
    };
    let published = Published(BTreeSet::from([old]));
    assert!(verify_publication(&dataset, &Aliases::default(), &published).is_err());
    let aliases = Aliases(BTreeMap::from([(old, middle), (middle, current)]));
    verify_publication(&dataset, &aliases, &published).unwrap();
    let cyclic = Aliases(BTreeMap::from([(old, middle), (middle, old)]));
    assert!(verify_publication(&dataset, &cyclic, &published).is_err());
    let shadows = Aliases(BTreeMap::from([(current, old)]));
    assert!(verify_publication(&dataset, &shadows, &published).is_err());
}
