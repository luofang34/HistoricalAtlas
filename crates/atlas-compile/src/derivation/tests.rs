use super::*;
use atlas_core::identity::EntityId;

#[test]
fn real_commune_recipe_publishes_every_reconstruction_with_its_source_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let request: DerivationRequest =
        read_json_blocking(&root.join("guizhou-1960s/bijie/communes.derive.json")).unwrap();
    let output = compile_derivation(&input, &request).unwrap();
    assert_eq!(request.units.len(), 124);
    assert!(
        output
            .values()
            .all(|b| b.len() < crate::export::MAX_FILE_BYTES)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &output[Path::new("derived/bijie-communes-terrain.json")]
        )
        .unwrap(),
        serde_json::to_value(&request).unwrap()
    );
    for unit in &request.units {
        let record = input
            .data
            .records
            .iter()
            .find(|r| r.id == unit.record_id)
            .unwrap();
        let path = format!("entities/index/{}.json", record.entity_id.shard());
        let shard: serde_json::Value = serde_json::from_slice(&output[Path::new(&path)]).unwrap();
        let boundary: DerivedBoundary = serde_json::from_value(
            shard[record.entity_id.0.to_string()]["derived_boundaries"][0].clone(),
        )
        .unwrap();
        let feature = boundary.feature(&input.data, None).unwrap();
        assert_eq!(feature["properties"]["reconstructed"], true);
        assert_eq!(feature["properties"]["geometry_kind"], "derived_polygon");
        assert!(feature["properties"]["validity"].is_null());
        assert!(boundary.basis.contains(&request.envelope_record_id));
        assert!(boundary.basis.contains(&unit.record_id));
        assert!(boundary.region.cells > 0);
    }
    let id = EntityId::from_key("earth/cn-gz/bijie/hantun-commune").unwrap();
    let unit = request
        .units
        .iter()
        .find(|u| {
            input
                .data
                .records
                .iter()
                .any(|r| r.id == u.record_id && r.entity_id == id)
        })
        .unwrap();
    assert!(
        unit.anchors
            .iter()
            .any(|a| matches!(a, AnchorReference::MapLocation { .. }))
    );
    assert!(!request.excluded.is_empty());
}

#[test]
fn reconstruction_rejects_other_communes_anchors_and_uncited_exclusions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let request: DerivationRequest =
        read_json_blocking(&root.join("guizhou-1960s/bijie/communes.derive.json")).unwrap();
    let mut invalid = request.clone();
    invalid.units[0].anchors = invalid.units[1].anchors.clone();
    assert!(
        reconstruct(&input, &invalid)
            .unwrap_err()
            .to_string()
            .contains("hierarchy path")
    );
    let mut invalid = request.clone();
    invalid.excluded[0].reason.clear();
    assert!(
        reconstruct(&input, &invalid)
            .unwrap_err()
            .to_string()
            .contains("reason")
    );
    let mut invalid = request;
    invalid.terrain_tiles[0].sha256 = "invalid".into();
    assert!(
        reconstruct(&input, &invalid)
            .unwrap_err()
            .to_string()
            .contains("fingerprint")
    );
}

#[test]
fn reconstruction_rejects_a_seed_whose_review_has_been_withdrawn() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    let request: DerivationRequest =
        read_json_blocking(&root.join("guizhou-1960s/bijie/communes.derive.json")).unwrap();
    let seed = request
        .units
        .iter()
        .flat_map(|unit| &unit.anchors)
        .find_map(|anchor| match anchor {
            AnchorReference::Record { record_id } => Some(*record_id),
            AnchorReference::MapLocation { .. } => None,
        })
        .unwrap();
    input
        .data
        .records
        .iter_mut()
        .find(|r| r.id == seed)
        .unwrap()
        .checked = false;
    assert!(
        reconstruct(&input, &request)
            .unwrap_err()
            .to_string()
            .contains("missing checked")
    );
}

#[test]
fn source_comparisons_and_the_haogou_exclusion_follow_the_reconstructed_geometry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let request = &input.derivations["bijie-communes-terrain"];
    let boundaries = reconstruct(&input, request).unwrap();
    for (slug, area, neighbors) in [
        ("baiyan", 12.5, 4),
        ("haogou", 9.5, 5),
        ("huangnichong", 47.9, 6),
    ] {
        let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{slug}-commune")).unwrap();
        let boundary = boundaries
            .iter()
            .find(|b| {
                input
                    .data
                    .records
                    .iter()
                    .any(|r| r.id == b.record_id && r.entity_id == id)
            })
            .unwrap();
        assert_eq!(boundary.comparisons.len(), 1);
        let comparison = &boundary.comparisons[0];
        assert_eq!(comparison.reported_area_square_km, area);
        assert_eq!(comparison.fitting_tolerance_square_km, Some(0.1));
        assert!(comparison.modeled_area_square_km > 0.0);
        assert!((comparison.modeled_area_square_km - area).abs() <= 0.1);
        assert_eq!(comparison.neighbors.len(), neighbors);
        assert!(boundary.basis.contains(&comparison.record_id));
        assert!(
            comparison
                .neighbors
                .iter()
                .filter(|n| n.entity_id.is_some())
                .all(|n| n.shares_edge.is_some())
        );
        let mut changed = boundary.clone();
        changed.comparisons[0].modeled_area_square_km *= 2.0;
        assert!(changed.feature(&input.data, None).is_err());
        let mut invalid = boundary.clone();
        invalid.comparisons[0].fitting_tolerance_square_km = Some(0.);
        assert!(invalid.feature(&input.data, None).is_err());
        if slug == "haogou" || slug == "baiyan" {
            assert_eq!(boundary.region.polygons.len(), 1);
            assert!(boundary.region.vertices.iter().all(|p| p.x > 105.4));
        }
        if slug != "haogou" {
            assert!(
                comparison
                    .neighbors
                    .iter()
                    .all(|n| n.shares_edge != Some(false))
            );
        }
    }
    assert_printed_haogou_excluded(&input, request);
}

#[cfg(test)]
fn assert_printed_haogou_excluded(input: &LedgerInput, request: &DerivationRequest) {
    let printed = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/p188/haogou-commune")
        .unwrap();
    assert_eq!(
        printed
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .longitude_text,
        "105°20′"
    );
    assert!(
        request
            .excluded
            .iter()
            .any(|a| a.record_id == printed.id && a.reason.contains("105°28′"))
    );
    assert!(
        !request.units.iter().flat_map(|u| &u.anchors).any(
            |a| matches!(a, AnchorReference::Record { record_id } if *record_id == printed.id)
        )
    );
}

#[test]
fn area_constraints_reject_unreviewed_or_other_units_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    let mut request = input.derivations["bijie-communes-terrain"].clone();
    let constrained: Vec<_> = request
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.area_constraint.is_some())
        .map(|(i, _)| i)
        .collect();
    assert_eq!(constrained.len(), 3);
    let first = constrained[0];
    let constraint = request.units[first].area_constraint.clone().unwrap();
    request.units[first].area_constraint = request.units[constrained[1]].area_constraint.clone();
    assert!(
        reconstruct(&input, &request)
            .unwrap_err()
            .to_string()
            .contains("territory evidence")
    );
    request.units[first].area_constraint = Some(constraint.clone());
    input
        .data
        .records
        .iter_mut()
        .find(|r| r.id == constraint.record_id)
        .unwrap()
        .checked = false;
    assert!(
        reconstruct(&input, &request)
            .unwrap_err()
            .to_string()
            .contains("missing checked")
    );
}
