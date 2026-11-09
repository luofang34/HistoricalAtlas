use super::Application;
use atlas_boundary::publication::{GeometryBundle, GeometryReference};
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::mpsc,
};

type Loaded = (EntityId, Result<GeometryBundle, String>);

pub(super) struct GeometryLoads {
    references: BTreeMap<EntityId, (BodyId, GeometryReference)>,
    pending: BTreeSet<EntityId>,
    failed: BTreeSet<EntityId>,
    send: mpsc::Sender<Loaded>,
    receive: mpsc::Receiver<Loaded>,
}

impl GeometryLoads {
    pub(super) fn new(atlas: &crate::catalog::Atlas) -> Self {
        let (send, receive) = mpsc::channel();
        let references = [BodyId::Earth, BodyId::Moon, BodyId::Mars]
            .into_iter()
            .flat_map(|body| {
                atlas
                    .geometry_references(body)
                    .map(move |(id, reference)| (id, (body, reference.clone())))
            })
            .collect();
        Self {
            references,
            pending: BTreeSet::new(),
            failed: BTreeSet::new(),
            send,
            receive,
        }
    }
}

impl Application {
    pub(super) fn load_visible_geometry(&mut self, ui: &mut egui::Ui) {
        let needs_focus = self
            .selected
            .is_some_and(|id| self.focus_point(id).is_none());
        let mut changed = false;
        for (id, result) in self.geometry.receive.try_iter() {
            self.geometry.pending.remove(&id);
            match result.and_then(|bundle| {
                self.atlas
                    .attach_geometry(id, bundle)
                    .map_err(|e| e.to_string())
            }) {
                Ok(()) => {
                    self.geometry.references.remove(&id);
                    changed = true;
                }
                Err(error) => {
                    self.geometry.failed.insert(id);
                    self.error = Some(error);
                }
            }
        }
        if changed {
            self.dirty = true;
            if needs_focus && let Some(id) = self.selected {
                self.select(id, true);
            }
        }
        if !self.geometry.failed.is_empty() && ui.button("Retry map detail").clicked() {
            self.geometry.failed.clear();
        }
        let bounds = self.map.as_ref().and_then(|map| map.detail_bounds());
        let available = 8_usize.saturating_sub(self.geometry.pending.len());
        let requested: Vec<_> = self
            .geometry
            .references
            .iter()
            .filter(|(id, (body, reference))| {
                *body == self.body
                    && !self.geometry.pending.contains(id)
                    && !self.geometry.failed.contains(id)
                    && (self.selected.is_some_and(|selected| {
                        **id == selected || reference.subjects.contains(&selected)
                    }) || bounds
                        .zip(reference.bounds)
                        .is_some_and(|(view, detail)| intersects(view, detail)))
            })
            .take(available)
            .map(|(id, (_, reference))| (*id, reference.clone()))
            .collect();
        for (id, reference) in requested {
            self.geometry.pending.insert(id);
            let send = self.geometry.send.clone();
            let context = ui.ctx().clone();
            wasm_bindgen_futures::spawn_local(async move {
                send.send((id, fetch(reference).await)).ok();
                context.request_repaint();
            });
        }
        if !self.geometry.pending.is_empty() {
            ui.label("Loading map detail…");
        }
    }
}

async fn fetch(reference: GeometryReference) -> Result<GeometryBundle, String> {
    reference.validate().map_err(|e| e.to_string())?;
    let mut parts = Vec::new();
    for part in &reference.parts {
        parts.push(
            crate::resources::bytes(&format!("data/{}", part.path))
                .await
                .map_err(|e| e.to_string())?,
        );
    }
    reference.decode(&parts).map_err(|e| e.to_string())
}

fn intersects([w, s, e, n]: [f64; 4], [west, south, east, north]: [f64; 4]) -> bool {
    south <= n
        && north >= s
        && [-360., 0., 360.]
            .into_iter()
            .any(|shift| west + shift <= e && east + shift >= w)
}
