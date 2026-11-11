//! Structural jurisdiction and ontological category trees for browsing enduring entities.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;

use crate::catalog::Atlas;

mod panel;
mod taxonomy;
#[cfg(test)]
mod tests;

pub(super) use taxonomy::{DomainGroup, rank_label};
use taxonomy::{category_subkind, entity_badge, rank_order};

/// Structural organization mode in the `Places` browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum TreeMode {
    /// Parent-child administrative and spatial containment hierarchy, grouped at the root by domain.
    #[default]
    Hierarchy,
    /// Two-level ontological domain and subtype taxonomy across the active body.
    Domain,
}

#[derive(Clone, Debug)]
pub(super) enum TreeRow {
    Domain {
        domain: DomainGroup,
        count: usize,
        expanded: bool,
    },
    Subkind {
        domain: DomainGroup,
        subkind: String,
        count: usize,
        expanded: bool,
    },
    Entity {
        id: EntityId,
        depth: usize,
        child_count: usize,
        expanded: bool,
        badge: String,
    },
}

struct BodyHierarchy {
    body: BodyId,
    entity_count: usize,
    children: BTreeMap<EntityId, Vec<EntityId>>,
    parents: BTreeMap<EntityId, Vec<EntityId>>,
    roots: BTreeMap<DomainGroup, Vec<EntityId>>,
    categories: BTreeMap<DomainGroup, BTreeMap<String, Vec<EntityId>>>,
    domains: BTreeMap<EntityId, DomainGroup>,
    subkinds: BTreeMap<EntityId, String>,
    badges: BTreeMap<EntityId, String>,
}

/// Cached structural tree and expansion state for the `Places` panel.
#[derive(Default)]
pub(super) struct PlaceTree {
    pub(super) mode: TreeMode,
    index: Option<BodyHierarchy>,
    expanded_domains: BTreeSet<DomainGroup>,
    expanded_subkinds: BTreeSet<(DomainGroup, String)>,
    expanded_entities: BTreeSet<EntityId>,
    rows: Vec<TreeRow>,
    rows_dirty: bool,
    scroll_to: Option<EntityId>,
}

impl PlaceTree {
    pub(super) fn ensure_synced(&mut self, atlas: &Atlas, body: BodyId) {
        let count = atlas.body_entities(body).len();
        let current = self
            .index
            .as_ref()
            .is_some_and(|idx| idx.body == body && idx.entity_count == count);
        if !current {
            let built = build_body_hierarchy(atlas, body);
            self.apply_defaults(&built);
            self.index = Some(built);
            self.rows_dirty = true;
        }
        if self.rows_dirty {
            self.rebuild_rows();
        }
    }

    fn apply_defaults(&mut self, built: &BodyHierarchy) {
        self.expanded_domains.clear();
        self.expanded_subkinds.clear();
        self.expanded_entities.clear();
        if built.entity_count <= 24 {
            self.expanded_domains.extend(built.roots.keys().copied());
            self.expanded_domains
                .extend(built.categories.keys().copied());
            for (domain, submap) in &built.categories {
                for subkind in submap.keys() {
                    self.expanded_subkinds.insert((*domain, subkind.clone()));
                }
            }
            return;
        }
        if built.roots.contains_key(&DomainGroup::Polity) {
            self.expanded_domains.insert(DomainGroup::Polity);
        } else if let Some(&first) = built.roots.keys().next() {
            self.expanded_domains.insert(first);
        }
        if let Some(polity_roots) = built.roots.get(&DomainGroup::Polity) {
            for &root in polity_roots {
                let n = built.children.get(&root).map_or(0, Vec::len);
                if (1..=32).contains(&n) {
                    self.expanded_entities.insert(root);
                }
            }
        }
    }

    /// Expand ancestors (and `id` itself if it owns children) so `id` is visible in the tree.
    pub(super) fn reveal(&mut self, atlas: &Atlas, body: BodyId, id: EntityId, scroll: bool) {
        let Some(id) = atlas.resolve(id) else {
            return;
        };
        self.ensure_synced(atlas, body);
        let Some(index) = &self.index else {
            return;
        };
        if !index.domains.contains_key(&id) {
            return;
        }
        let mut expanded_domains = self.expanded_domains.clone();
        let mut expanded_subkinds = self.expanded_subkinds.clone();
        let mut expanded_entities = self.expanded_entities.clone();
        if let Some(&domain) = index.domains.get(&id) {
            if index.parents.get(&id).is_none_or(Vec::is_empty) {
                expanded_domains.insert(domain);
            }
            if let Some(subkind) = index.subkinds.get(&id) {
                expanded_domains.insert(domain);
                expanded_subkinds.insert((domain, subkind.clone()));
            }
        }
        if index.children.get(&id).is_some_and(|c| !c.is_empty()) {
            expanded_entities.insert(id);
        }
        let mut queue: VecDeque<EntityId> = index
            .parents
            .get(&id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut visited = BTreeSet::from([id]);
        while let Some(curr) = queue.pop_front() {
            if !visited.insert(curr) {
                continue;
            }
            expanded_entities.insert(curr);
            let parents = index.parents.get(&curr).map_or(&[][..], Vec::as_slice);
            if parents.is_empty()
                && let Some(&domain) = index.domains.get(&curr)
            {
                expanded_domains.insert(domain);
            }
            for &parent in parents {
                queue.push_back(parent);
            }
        }
        self.expanded_domains = expanded_domains;
        self.expanded_subkinds = expanded_subkinds;
        self.expanded_entities = expanded_entities;
        self.rows_dirty = true;
        if scroll {
            self.scroll_to = Some(id);
        }
    }

    /// Shortest ancestor path `[root, ..., parent]` for `id` on `body`.
    pub(super) fn ancestor_trail(
        &mut self,
        atlas: &Atlas,
        body: BodyId,
        id: EntityId,
    ) -> Vec<EntityId> {
        let Some(id) = atlas.resolve(id) else {
            return Vec::new();
        };
        self.ensure_synced(atlas, body);
        let Some(index) = &self.index else {
            return Vec::new();
        };
        let mut trail = Vec::new();
        let mut seen = BTreeSet::from([id]);
        let mut curr = id;
        while let Some(&parent) = index.parents.get(&curr).and_then(|p| p.first()) {
            if !seen.insert(parent) {
                break;
            }
            trail.push(parent);
            curr = parent;
        }
        trail.reverse();
        trail
    }

    pub(super) fn badge(&mut self, atlas: &Atlas, body: BodyId, id: EntityId) -> String {
        let Some(id) = atlas.resolve(id) else {
            return String::new();
        };
        self.ensure_synced(atlas, body);
        self.index
            .as_ref()
            .and_then(|idx| idx.badges.get(&id).cloned())
            .unwrap_or_default()
    }

    pub(super) fn set_mode(&mut self, mode: TreeMode) {
        if self.mode != mode {
            self.mode = mode;
            self.rows_dirty = true;
        }
    }

    pub(super) fn toggle_domain(&mut self, domain: DomainGroup) {
        if !self.expanded_domains.remove(&domain) {
            self.expanded_domains.insert(domain);
        }
        self.rows_dirty = true;
    }

    pub(super) fn toggle_subkind(&mut self, domain: DomainGroup, subkind: String) {
        let key = (domain, subkind);
        if !self.expanded_subkinds.remove(&key) {
            self.expanded_subkinds.insert(key);
        }
        self.rows_dirty = true;
    }

    pub(super) fn toggle_entity(&mut self, id: EntityId) {
        if !self.expanded_entities.remove(&id) {
            self.expanded_entities.insert(id);
        }
        self.rows_dirty = true;
    }

    pub(super) fn expand_entity(&mut self, id: EntityId) {
        if self.expanded_entities.insert(id) {
            self.rows_dirty = true;
        }
    }

    pub(super) fn take_scroll_row(&mut self) -> Option<usize> {
        let target = self.scroll_to.take()?;
        self.rows
            .iter()
            .position(|row| matches!(row, TreeRow::Entity { id, .. } if *id == target))
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        self.rows_dirty = false;
        let Some(index) = &self.index else {
            return;
        };
        match self.mode {
            TreeMode::Hierarchy => {
                for (&domain, roots) in &index.roots {
                    let expanded = self.expanded_domains.contains(&domain);
                    self.rows.push(TreeRow::Domain {
                        domain,
                        count: roots.len(),
                        expanded,
                    });
                    if expanded {
                        let mut branch = BTreeSet::new();
                        for &root in roots {
                            push_hierarchy_node(
                                index,
                                &self.expanded_entities,
                                root,
                                1,
                                &mut branch,
                                &mut self.rows,
                            );
                        }
                    }
                }
            }
            TreeMode::Domain => {
                for (&domain, submap) in &index.categories {
                    let total = submap.values().map(Vec::len).sum();
                    let expanded = self.expanded_domains.contains(&domain);
                    self.rows.push(TreeRow::Domain {
                        domain,
                        count: total,
                        expanded,
                    });
                    if !expanded {
                        continue;
                    }
                    for (subkind, ids) in submap {
                        let open = self.expanded_subkinds.contains(&(domain, subkind.clone()));
                        self.rows.push(TreeRow::Subkind {
                            domain,
                            subkind: subkind.clone(),
                            count: ids.len(),
                            expanded: open,
                        });
                        if open {
                            for &id in ids {
                                let child_count = index.children.get(&id).map_or(0, Vec::len);
                                let badge = index.badges.get(&id).cloned().unwrap_or_default();
                                self.rows.push(TreeRow::Entity {
                                    id,
                                    depth: 2,
                                    child_count,
                                    expanded: false,
                                    badge,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn push_hierarchy_node(
    index: &BodyHierarchy,
    expanded_entities: &BTreeSet<EntityId>,
    id: EntityId,
    depth: usize,
    branch: &mut BTreeSet<EntityId>,
    out: &mut Vec<TreeRow>,
) {
    if !branch.insert(id) {
        return;
    }
    let children = index.children.get(&id).map_or(&[][..], Vec::as_slice);
    let expanded = !children.is_empty() && expanded_entities.contains(&id);
    let badge = index.badges.get(&id).cloned().unwrap_or_default();
    out.push(TreeRow::Entity {
        id,
        depth,
        child_count: children.len(),
        expanded,
        badge,
    });
    if expanded {
        for &child in children {
            push_hierarchy_node(
                index,
                expanded_entities,
                child,
                depth.saturating_add(1),
                branch,
                out,
            );
        }
    }
    branch.remove(&id);
}

fn build_body_hierarchy(atlas: &Atlas, body: BodyId) -> BodyHierarchy {
    let ids = atlas.body_entities(body);
    let mut children: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    let mut parents: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    for (parent, child) in atlas.parent_links(body) {
        children.entry(parent).or_default().push(child);
        parents.entry(child).or_default().push(parent);
    }
    let mut domains = BTreeMap::new();
    let mut subkinds = BTreeMap::new();
    let mut badges = BTreeMap::new();
    let mut sort_keys = BTreeMap::new();
    for &id in &ids {
        let kind = atlas.entity_kind(id);
        let ranks = atlas.entity_ranks(id);
        let domain = DomainGroup::from_kind(kind);
        let badge = entity_badge(kind, &ranks);
        let subkind = category_subkind(kind, &ranks);
        let rank_ord = ranks.iter().copied().map(rank_order).min().unwrap_or(99);
        let has_children = children.get(&id).is_some_and(|c| !c.is_empty());
        let key = atlas.entity_key(id).unwrap_or("").to_owned();
        domains.insert(id, domain);
        subkinds.insert(id, subkind);
        badges.insert(id, badge);
        sort_keys.insert(id, (rank_ord, !has_children, domain, key));
    }
    let cmp_id = |a: &EntityId, b: &EntityId| sort_keys.get(a).cmp(&sort_keys.get(b));
    for list in children.values_mut() {
        list.sort_by(cmp_id);
        list.dedup();
    }
    for list in parents.values_mut() {
        list.sort_by(cmp_id);
        list.dedup();
    }
    let mut roots: BTreeMap<DomainGroup, Vec<EntityId>> = BTreeMap::new();
    let mut categories: BTreeMap<DomainGroup, BTreeMap<String, Vec<EntityId>>> = BTreeMap::new();
    for &id in &ids {
        let domain = domains.get(&id).copied().unwrap_or(DomainGroup::Place);
        if parents.get(&id).is_none_or(Vec::is_empty) {
            roots.entry(domain).or_default().push(id);
        }
        let sub = subkinds.get(&id).cloned().unwrap_or_default();
        categories
            .entry(domain)
            .or_default()
            .entry(sub)
            .or_default()
            .push(id);
    }
    for list in roots.values_mut() {
        list.sort_by(cmp_id);
    }
    for submap in categories.values_mut() {
        for list in submap.values_mut() {
            list.sort_by(cmp_id);
        }
    }
    BodyHierarchy {
        body,
        entity_count: ids.len(),
        children,
        parents,
        roots,
        categories,
        domains,
        subkinds,
        badges,
    }
}
