use super::*;
use atlas_core::identity::EntityId;
use std::path::Path;

#[cfg(test)]
fn index(input: &LedgerInput, request: &DerivationRequest, key: &str) -> usize {
    let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{key}-commune")).unwrap();
    request
        .units
        .iter()
        .position(|u| {
            input
                .data
                .records
                .iter()
                .any(|r| r.id == u.record_id && r.entity_id == id)
        })
        .unwrap()
}

#[test]
fn reviewed_conflict_retains_its_evidence_and_other_neighbor_requirements() {
    let input =
        crate::input::load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap();
    let request = &input.derivations["bijie-communes-terrain"];
    let zongji = index(&input, request, "zongji");
    let daji = index(&input, request, "daji");
    let pair = |a: usize, b: usize| [a.min(b), a.max(b)];
    let mut bases = vec![BTreeSet::new(); request.units.len()];
    let pairs = resolve(&input, request, &mut bases).unwrap();
    assert!(!pairs.contains(&pair(zongji, daji)));
    for neighbor in ["wangjiaba", "shisanmu", "lanni", "cengtai", "yongxing"] {
        assert!(pairs.contains(&pair(zongji, index(&input, request, neighbor))));
    }
    let constraint = request.units[zongji].area_constraint.as_ref().unwrap();
    let review = &constraint.unresolved_neighbors[0];
    for unit in [zongji, daji] {
        assert!(review.basis.iter().all(|id| bases[unit].contains(id)));
    }
    let mut restored = request.clone();
    restored.units[zongji]
        .area_constraint
        .as_mut()
        .unwrap()
        .unresolved_neighbors
        .clear();
    assert!(
        resolve(&input, &restored, &mut bases)
            .unwrap()
            .contains(&pair(zongji, daji))
    );
}

#[test]
fn malformed_duplicate_and_unchecked_neighbor_reviews_are_rejected() {
    let input =
        crate::input::load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap();
    let request = &input.derivations["bijie-communes-terrain"];
    let zongji = index(&input, request, "zongji");
    for fault in 0..6 {
        let mut invalid = request.clone();
        let reviews = &mut invalid.units[zongji]
            .area_constraint
            .as_mut()
            .unwrap()
            .unresolved_neighbors;
        match fault {
            0 => reviews[0].reason.clear(),
            1 => {
                reviews[0].basis.remove(0);
            }
            2 => {
                let id = reviews[0].basis[0];
                reviews[0].basis.push(id);
            }
            3 => reviews[0].basis[1] = FactId::from_key("test/missing-neighbor-proof").unwrap(),
            4 => {
                reviews[0].entity_id =
                    EntityId::from_key("earth/cn-gz/bijie/xiaoji-commune").unwrap()
            }
            _ => reviews.push(reviews[0].clone()),
        }
        assert!(
            resolve(
                &input,
                &invalid,
                &mut vec![BTreeSet::new(); invalid.units.len()]
            )
            .is_err()
        );
    }
    let mut unchecked =
        crate::input::load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap();
    let id = request.units[zongji]
        .area_constraint
        .as_ref()
        .unwrap()
        .unresolved_neighbors[0]
        .basis[2];
    unchecked
        .data
        .records
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .checked = false;
    assert!(
        resolve(
            &unchecked,
            request,
            &mut vec![BTreeSet::new(); request.units.len()]
        )
        .is_err()
    );
}
