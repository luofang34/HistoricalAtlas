use atlas_core::identity::EntityId;

use super::{DomainGroup, TreeMode, TreeRow};
use crate::application::Application;

enum TreeAction {
    ToggleDomain(DomainGroup),
    ToggleSubkind(DomainGroup, String),
    ToggleEntity(EntityId),
    SelectEntity(EntityId, bool),
}

fn paint_disclosure(ui: &egui::Ui, rect: egui::Rect, expanded: bool, color: egui::Color32) {
    let center = rect.center();
    let r = 3.8;
    let points = if expanded {
        vec![
            egui::pos2(center.x - r, center.y - r * 0.55),
            egui::pos2(center.x + r, center.y - r * 0.55),
            egui::pos2(center.x, center.y + r * 0.8),
        ]
    } else {
        vec![
            egui::pos2(center.x - r * 0.55, center.y - r),
            egui::pos2(center.x + r * 0.8, center.y),
            egui::pos2(center.x - r * 0.55, center.y + r),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

impl Application {
    pub(in crate::application) fn places_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Places");
        ui.horizontal(|ui| {
            let width = if self.search.is_empty() { 230.0 } else { 190.0 };
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(width)
                    .hint_text("Name, pinyin, former name, UUID"),
            );
            if !self.search.is_empty() && ui.small_button("Clear").clicked() {
                self.search.clear();
                if let Some(id) = self.selected {
                    self.tree.reveal(&self.atlas, self.body, id, true);
                }
            }
        });
        self.map_hits_panel(ui);
        if self.search.trim().is_empty() {
            self.structural_tree_panel(ui);
        } else {
            self.filtered_search_panel(ui);
        }
    }

    fn map_hits_panel(&mut self, ui: &mut egui::Ui) {
        if self.hits.is_empty() {
            return;
        }
        ui.strong("At this map point");
        let mut chosen = None;
        for id in self.hits.clone() {
            let badge = self.tree.badge(&self.atlas, self.body, id);
            let label = if badge.is_empty() {
                self.label(id)
            } else {
                format!("{} · {badge}", self.label(id))
            };
            if ui
                .selectable_label(self.selected == Some(id), label)
                .clicked()
            {
                chosen = Some(id);
            }
        }
        if let Some(id) = chosen {
            self.select_and_reveal(id, false, true);
        }
        ui.separator();
    }

    fn structural_tree_panel(&mut self, ui: &mut egui::Ui) {
        self.tree.ensure_synced(&self.atlas, self.body);
        let total = self.atlas.body_entities(self.body).len();
        ui.horizontal_wrapped(|ui| {
            let mut mode = self.tree.mode;
            if ui
                .selectable_value(&mut mode, TreeMode::Hierarchy, "Jurisdictions")
                .clicked()
                || ui
                    .selectable_value(&mut mode, TreeMode::Domain, "Categories")
                    .clicked()
            {
                self.tree.set_mode(mode);
                if let Some(id) = self.selected {
                    self.tree.reveal(&self.atlas, self.body, id, true);
                }
                self.tree.ensure_synced(&self.atlas, self.body);
            }
            ui.weak(format!("{total} entities"));
        });
        let row_height = ui.spacing().interact_size.y;
        let row_pitch = row_height + ui.spacing().item_spacing.y;
        let scroll_row = self.tree.take_scroll_row();
        let row_count = self.tree.rows.len();
        let body_salt = match self.body {
            atlas_geo::body::BodyId::Earth => 0_u8,
            atlas_geo::body::BodyId::Moon => 1_u8,
            atlas_geo::body::BodyId::Mars => 2_u8,
        };
        let mut area = egui::ScrollArea::vertical()
            .id_salt(("places-tree", body_salt, self.tree.mode))
            .auto_shrink([false, false]);
        if let Some(index) = scroll_row {
            area = area.vertical_scroll_offset((index.saturating_sub(3) as f32) * row_pitch);
        }
        let mut action = None;
        area.show_rows(ui, row_height, row_count, |ui, range| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            let visible: Vec<TreeRow> = self
                .tree
                .rows
                .get(range)
                .map_or_else(Vec::new, |slice| slice.to_vec());
            for row in visible {
                if let Some(next) = self.draw_tree_row(ui, row) {
                    action = Some(next);
                }
            }
        });
        self.apply_tree_action(action);
    }

    fn draw_tree_row(&self, ui: &mut egui::Ui, row: TreeRow) -> Option<TreeAction> {
        match row {
            TreeRow::Domain {
                domain,
                count,
                expanded,
            } => {
                let mut toggled = false;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let (icon_rect, icon_resp) =
                        ui.allocate_exact_size(egui::vec2(14.0, 16.0), egui::Sense::click());
                    paint_disclosure(ui, icon_rect, expanded, ui.visuals().strong_text_color());
                    let text = format!("{} ({count})", domain.title());
                    let label_resp = ui.selectable_label(false, egui::RichText::new(text).strong());
                    toggled = icon_resp.clicked() || label_resp.clicked();
                });
                toggled.then_some(TreeAction::ToggleDomain(domain))
            }
            TreeRow::Subkind {
                domain,
                subkind,
                count,
                expanded,
            } => {
                let mut out = None;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add_space(12.0);
                    let (icon_rect, icon_resp) =
                        ui.allocate_exact_size(egui::vec2(14.0, 16.0), egui::Sense::click());
                    paint_disclosure(ui, icon_rect, expanded, ui.visuals().text_color());
                    let text = format!("{subkind} ({count})");
                    if icon_resp.clicked() || ui.selectable_label(false, text).clicked() {
                        out = Some(TreeAction::ToggleSubkind(domain, subkind));
                    }
                });
                out
            }
            TreeRow::Entity {
                id,
                depth,
                child_count,
                expanded,
                badge,
            } => self.draw_entity_row(ui, id, depth, child_count, expanded, &badge),
        }
    }

    fn draw_entity_row(
        &self,
        ui: &mut egui::Ui,
        id: EntityId,
        depth: usize,
        child_count: usize,
        expanded: bool,
        badge: &str,
    ) -> Option<TreeAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space((depth as f32) * 12.0);
            if child_count > 0 {
                let (icon_rect, icon_resp) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                if icon_resp.hovered() {
                    ui.painter().rect_filled(
                        icon_rect.shrink(1.0),
                        3.0,
                        ui.visuals().widgets.hovered.bg_fill,
                    );
                }
                paint_disclosure(ui, icon_rect, expanded, ui.visuals().text_color());
                if icon_resp.clicked() {
                    action = Some(TreeAction::ToggleEntity(id));
                }
            } else {
                ui.add_space(16.0);
            }
            let suffix = match (!badge.is_empty(), child_count > 0) {
                (true, true) => format!(" · {badge} ({child_count})"),
                (true, false) => format!(" · {badge}"),
                (false, true) => format!(" ({child_count})"),
                (false, false) => String::new(),
            };
            let mut job = egui::text::LayoutJob::default();
            let base_color = if self.selected == Some(id) {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().text_color()
            };
            job.append(
                &self.label(id),
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::proportional(13.5),
                    color: base_color,
                    ..Default::default()
                },
            );
            if !suffix.is_empty() {
                job.append(
                    &suffix,
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(11.5),
                        color: ui.visuals().weak_text_color(),
                        ..Default::default()
                    },
                );
            }
            let response = ui.selectable_label(self.selected == Some(id), job);
            if response.clicked() {
                action = Some(TreeAction::SelectEntity(id, child_count > 0));
            }
            response.on_hover_text(self.atlas.entity_key(id).unwrap_or(""));
        });
        action
    }

    fn apply_tree_action(&mut self, action: Option<TreeAction>) {
        match action {
            Some(TreeAction::ToggleDomain(domain)) => self.tree.toggle_domain(domain),
            Some(TreeAction::ToggleSubkind(domain, subkind)) => {
                self.tree.toggle_subkind(domain, subkind);
            }
            Some(TreeAction::ToggleEntity(id)) => self.tree.toggle_entity(id),
            Some(TreeAction::SelectEntity(id, has_children)) => {
                if has_children {
                    self.tree.expand_entity(id);
                }
                self.select_and_reveal(id, true, false);
            }
            None => {}
        }
    }

    fn filtered_search_panel(&mut self, ui: &mut egui::Ui) {
        let current = self
            .search_results
            .as_ref()
            .is_some_and(|(query, body, _)| *query == self.search && *body == self.body);
        if !current {
            let ids = self.atlas.search(&self.search, self.body);
            self.search_results = Some((self.search.clone(), self.body, ids));
        }
        let ids = self
            .search_results
            .as_ref()
            .map(|(_, _, ids)| ids.clone())
            .unwrap_or_default();
        let mut show_in_tree = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} matches", ids.len()));
            if let Some(id) = self.selected
                && ui.small_button("Show in tree").clicked()
            {
                self.search.clear();
                self.tree.reveal(&self.atlas, self.body, id, true);
                show_in_tree = true;
            }
        });
        if show_in_tree {
            return;
        }
        let row_height = ui.spacing().interact_size.y;
        let mut picked = None;
        egui::ScrollArea::vertical()
            .id_salt("search-results")
            .auto_shrink([false, false])
            .show_rows(ui, row_height, ids.len(), |ui, rows| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                for &id in ids.get(rows).unwrap_or_default() {
                    let trail = self.tree.ancestor_trail(&self.atlas, self.body, id);
                    let context = if trail.is_empty() {
                        self.tree.badge(&self.atlas, self.body, id)
                    } else {
                        trail
                            .into_iter()
                            .map(|ancestor| self.atlas.label(ancestor))
                            .collect::<Vec<_>>()
                            .join(" › ")
                    };
                    let text = if context.is_empty() {
                        self.label(id)
                    } else {
                        format!("{} · {context}", self.label(id))
                    };
                    let response = ui.selectable_label(self.selected == Some(id), text);
                    if response.clicked() {
                        picked = Some(id);
                    }
                    response.on_hover_text(self.atlas.entity_key(id).unwrap_or(""));
                }
            });
        if let Some(id) = picked {
            self.select_and_reveal(id, true, true);
        }
    }
}
