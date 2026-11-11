use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;

use super::{DomainGroup, PlaceTree, TreeMode, TreeRow};
use crate::application::{Application, configure_fonts_blocking};

#[test]
fn earth_places_arrange_into_jurisdiction_tree_and_auto_expand_selected_divisions() {
    let atlas = crate::catalog::tests::atlas();
    let county = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    let commune = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    let mut tree = PlaceTree::default();
    tree.ensure_synced(&atlas, BodyId::Earth);

    // Root Polities domain and county root are expanded by default on startup.
    assert!(tree.rows.iter().any(|row| matches!(
        row,
        TreeRow::Domain {
            domain: DomainGroup::Polity,
            expanded: true,
            ..
        }
    )));
    assert!(tree.rows.iter().any(|row| matches!(
        row,
        TreeRow::Entity {
            id,
            depth: 1,
            expanded: true,
            ..
        } if *id == county
    )));

    // Revealing a commune (as when clicking its polygon or marker on the map) unfolds its
    // ancestor district, unfolds the commune's own subordinate brigades/places, and targets its row.
    tree.reveal(&atlas, BodyId::Earth, commune, true);
    tree.ensure_synced(&atlas, BodyId::Earth);
    let scroll_row = tree.take_scroll_row().unwrap();
    match &tree.rows[scroll_row] {
        TreeRow::Entity {
            id,
            depth,
            child_count,
            expanded,
            badge,
        } => {
            assert_eq!(*id, commune);
            assert!(*depth >= 2);
            assert!(*child_count > 0);
            assert!(*expanded);
            assert_eq!(badge, "commune");
        }
        other => panic!("expected commune entity row, got {other:?}"),
    }
    // Immediate children of the commune appear right below it at depth + 1.
    assert!(matches!(
        &tree.rows[scroll_row + 1],
        TreeRow::Entity { depth, .. } if *depth >= 3
    ));
}

#[test]
fn category_mode_groups_all_entities_by_domain_and_subkind() {
    let atlas = crate::catalog::tests::atlas();
    let fauna = EntityId::from_key("earth/cn-gz/panxian/yangjuan-fauna").unwrap();
    let mut tree = PlaceTree::default();
    tree.set_mode(TreeMode::Domain);
    tree.reveal(&atlas, BodyId::Earth, fauna, true);
    tree.ensure_synced(&atlas, BodyId::Earth);
    let row = tree.take_scroll_row().unwrap();
    assert!(matches!(
        &tree.rows[row],
        TreeRow::Entity { id, depth: 2, .. } if *id == fauna
    ));
}

#[test]
fn selecting_a_division_renders_open_hierarchy_and_original_source_citation() {
    let atlas = crate::catalog::tests::atlas();
    let commune = EntityId::from_key("earth/cn-gz/bijie/cuiping-commune").unwrap();
    let mut app = Application::new(atlas).unwrap();
    app.select(commune, false);

    let context = egui::Context::default();
    configure_fonts_blocking(&context).unwrap();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440.0, 900.0),
            )),
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
    let rendered_texts: Vec<&str> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => Some(text.galley.job.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(rendered_texts.contains(&"Source hierarchy"));
    assert!(rendered_texts.contains(&"Source observations"));
    assert!(rendered_texts.contains(&"毕节县"));
    assert!(
        rendered_texts
            .iter()
            .any(|text| text.contains("Subordinate entities"))
    );
    assert!(
        rendered_texts
            .iter()
            .any(|text| text.contains("Printed page:"))
    );
}
