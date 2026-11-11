//! On-demand spatiotemporal and plate-rotated shard and geometry streaming.

#[cfg(any(target_arch = "wasm32", test))]
use std::collections::BTreeMap;

#[cfg(any(target_arch = "wasm32", test))]
use atlas_boundary::publication::ShardSummary;
#[cfg(any(target_arch = "wasm32", test))]
use atlas_core::time::EpochTime;
#[cfg(any(target_arch = "wasm32", test))]
use atlas_geo::{
    coordinates::LonLat,
    plates::{PlateId, RotationModel},
};
#[cfg(any(target_arch = "wasm32", test))]
use atlas_ledger::plates::PlateModelDocument;

#[cfg(any(target_arch = "wasm32", test))]
use crate::scene::MapLayer;

/// Rectangular geographic envelope intersection across antimeridian shifts.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn intersects([w, s, e, n]: [f64; 4], [west, south, east, north]: [f64; 4]) -> bool {
    south <= n
        && north >= s
        && [-360., 0., 360.]
            .into_iter()
            .any(|shift| west + shift <= e && east + shift >= w)
}

/// Whether a detail shard should stream for the current map layer, epoch, and viewport.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn shard_matches_view(
    shard: &ShardSummary,
    view_bounds: Option<[f64; 4]>,
    layer: MapLayer,
    at: EpochTime,
    plate_models: &BTreeMap<String, PlateModelDocument>,
    rotation_models: &BTreeMap<String, RotationModel>,
) -> bool {
    if layer == MapLayer::SelectedTime && !shard.temporal.matches_selected_time(at, plate_models) {
        return false;
    }
    let Some(bounds) = shard.bounds else {
        return false;
    };
    let Some(view) = view_bounds else {
        return layer == MapLayer::SelectedTime && !shard.temporal.plates.is_empty();
    };
    if layer != MapLayer::SelectedTime || shard.temporal.plates.is_empty() {
        return intersects(view, bounds);
    }
    if shard.temporal.has_temporal && intersects(view, bounds) {
        return true;
    }
    shard.temporal.plates.iter().any(|plate| {
        let Some(age_ma) = plate_models
            .get(&plate.model_id)
            .and_then(|doc| doc.age_at(at))
        else {
            return false;
        };
        rotation_models
            .get(&plate.model_id)
            .and_then(|model| rotated_bounds(model, plate.plate_id, age_ma, bounds))
            .is_some_and(|rotated| intersects(view, rotated))
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn rotated_bounds(
    model: &RotationModel,
    plate_id: PlateId,
    age_ma: f64,
    [w, s, e, n]: [f64; 4],
) -> Option<[f64; 4]> {
    let mid_lon = (w + e) * 0.5;
    let mid_lat = (s + n) * 0.5;
    let center = model
        .reconstruct(
            LonLat {
                longitude: mid_lon,
                latitude: mid_lat,
            },
            plate_id,
            age_ma,
            PlateId(0),
        )
        .ok()?;
    let mut envelope = [
        center.longitude,
        center.latitude,
        center.longitude,
        center.latitude,
    ];
    for lon in [w, mid_lon, e] {
        for lat in [s, mid_lat, n] {
            if let Ok(point) = model.reconstruct(
                LonLat {
                    longitude: lon,
                    latitude: lat,
                },
                plate_id,
                age_ma,
                PlateId(0),
            ) {
                let unwrapped = center.longitude
                    + (point.longitude - center.longitude + 180.0).rem_euclid(360.0)
                    - 180.0;
                let [min_lon, min_lat, max_lon, max_lat] = envelope;
                envelope = [
                    min_lon.min(unwrapped),
                    min_lat.min(point.latitude),
                    max_lon.max(unwrapped),
                    max_lat.max(point.latitude),
                ];
            }
        }
    }
    Some(envelope)
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::mpsc,
    };

    use atlas_boundary::publication::{GeometryBundle, GeometryReference, ShardSummary};
    use atlas_core::identity::EntityId;
    use atlas_geo::body::BodyId;

    use super::{intersects, shard_matches_view};
    use crate::application::Application;

    enum StreamOutcome {
        Shard(String, Result<Vec<u8>, String>),
        Geometry(EntityId, Result<GeometryBundle, String>),
    }

    pub(in crate::application) struct StreamingLoads {
        geometries: BTreeMap<EntityId, (BodyId, GeometryReference)>,
        pending_shards: BTreeSet<String>,
        failed_shards: BTreeSet<String>,
        pending_geometries: BTreeSet<EntityId>,
        failed_geometries: BTreeSet<EntityId>,
        send: mpsc::Sender<StreamOutcome>,
        receive: mpsc::Receiver<StreamOutcome>,
    }

    impl StreamingLoads {
        pub(in crate::application) fn new(atlas: &crate::catalog::Atlas) -> Self {
            let (send, receive) = mpsc::channel();
            let geometries = [BodyId::Earth, BodyId::Moon, BodyId::Mars]
                .into_iter()
                .flat_map(|body| {
                    atlas
                        .geometry_references(body)
                        .map(move |(id, reference)| (id, (body, reference.clone())))
                })
                .collect();
            Self {
                geometries,
                pending_shards: BTreeSet::new(),
                failed_shards: BTreeSet::new(),
                pending_geometries: BTreeSet::new(),
                failed_geometries: BTreeSet::new(),
                send,
                receive,
            }
        }
    }

    impl Application {
        pub(in crate::application) fn load_visible_streams(&mut self, ui: &mut egui::Ui) {
            let needs_focus = self
                .selected
                .is_some_and(|id| self.focus_point(id).is_none());
            let (changed, refresh_time) = self.drain_stream_results();
            if refresh_time
                && let Ok(time) = crate::selection::SelectedTime::query(&self.atlas, &self.time)
            {
                self.selected_time = time;
            }
            if changed {
                self.dirty = true;
                self.selection_dirty = true;
                if needs_focus && let Some(id) = self.selected {
                    self.select(id, true);
                }
            }
            if (!self.streaming.failed_shards.is_empty()
                || !self.streaming.failed_geometries.is_empty())
                && ui.button("Retry map detail").clicked()
            {
                self.streaming.failed_shards.clear();
                self.streaming.failed_geometries.clear();
            }
            self.schedule_streams(ui);
            if !self.streaming.pending_shards.is_empty()
                || !self.streaming.pending_geometries.is_empty()
            {
                ui.label("Loading map detail…");
            }
        }

        fn drain_stream_results(&mut self) -> (bool, bool) {
            let mut changed = false;
            let mut refresh_time = false;
            for message in self.streaming.receive.try_iter() {
                match message {
                    StreamOutcome::Shard(id, result) => {
                        self.streaming.pending_shards.remove(&id);
                        match result.and_then(|bytes| {
                            self.atlas
                                .attach_shard(&id, &bytes)
                                .map_err(|e| e.to_string())
                        }) {
                            Ok(attached) => {
                                refresh_time |= attached.refresh_time;
                                for (entity_id, body, reference) in attached.geometries {
                                    self.streaming
                                        .geometries
                                        .insert(entity_id, (body, reference));
                                }
                                changed = true;
                            }
                            Err(error) => {
                                self.streaming.failed_shards.insert(id);
                                self.error = Some(error);
                            }
                        }
                    }
                    StreamOutcome::Geometry(id, result) => {
                        self.streaming.pending_geometries.remove(&id);
                        match result.and_then(|bundle| {
                            self.atlas
                                .attach_geometry(id, bundle)
                                .map_err(|e| e.to_string())
                        }) {
                            Ok(()) => {
                                self.streaming.geometries.remove(&id);
                                changed = true;
                            }
                            Err(error) => {
                                self.streaming.failed_geometries.insert(id);
                                self.error = Some(error);
                            }
                        }
                    }
                }
            }
            (changed, refresh_time)
        }

        fn schedule_streams(&mut self, ui: &egui::Ui) {
            let in_flight = self
                .streaming
                .pending_shards
                .len()
                .saturating_add(self.streaming.pending_geometries.len());
            let mut available = 8_usize.saturating_sub(in_flight);
            if available == 0 {
                return;
            }
            let bounds = self.map.as_ref().and_then(|map| map.detail_bounds());
            let pinned_shards = self.priority_shards();
            let mut candidate_shards: Vec<ShardSummary> = self
                .atlas
                .unloaded_shards(self.body)
                .filter(|shard| {
                    self.atlas.is_shard_ready(shard)
                        && !self.streaming.pending_shards.contains(&shard.id)
                        && !self.streaming.failed_shards.contains(&shard.id)
                        && (shard.overview
                            || pinned_shards.contains(&shard.id)
                            || shard_matches_view(
                                shard,
                                bounds,
                                self.layer,
                                self.selected_time.at,
                                &self.atlas.plate_models,
                                self.atlas.rotation_models(),
                            ))
                })
                .cloned()
                .collect();
            candidate_shards.sort_by_key(|shard| {
                (
                    !shard.overview,
                    !pinned_shards.contains(&shard.id),
                    shard.id.clone(),
                )
            });
            for shard in candidate_shards.into_iter().take(available) {
                available = available.saturating_sub(1);
                let id = shard.id.clone();
                let path = shard.part.path.clone();
                self.streaming.pending_shards.insert(id.clone());
                let send = self.streaming.send.clone();
                let context = ui.ctx().clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let outcome = crate::resources::bytes(&format!("data/{path}"))
                        .await
                        .map_err(|e| e.to_string());
                    send.send(StreamOutcome::Shard(id, outcome)).ok();
                    context.request_repaint();
                });
            }
            if available == 0 || self.atlas.has_unloaded_overview_shards(self.body) {
                return;
            }
            self.schedule_geometries(ui, bounds, available);
        }

        fn schedule_geometries(
            &mut self,
            ui: &egui::Ui,
            bounds: Option<[f64; 4]>,
            available: usize,
        ) {
            let geometries: Vec<_> = self
                .streaming
                .geometries
                .iter()
                .filter(|(id, (body, reference))| {
                    *body == self.body
                        && !self.streaming.pending_geometries.contains(id)
                        && !self.streaming.failed_geometries.contains(id)
                        && (self.selected.is_some_and(|selected| {
                            **id == selected || reference.subjects.contains(&selected)
                        }) || bounds
                            .zip(reference.bounds)
                            .is_some_and(|(view, detail)| intersects(view, detail)))
                })
                .take(available)
                .map(|(id, (_, reference))| (*id, reference.clone()))
                .collect();
            for (id, reference) in geometries {
                self.streaming.pending_geometries.insert(id);
                let send = self.streaming.send.clone();
                let context = ui.ctx().clone();
                wasm_bindgen_futures::spawn_local(async move {
                    send.send(StreamOutcome::Geometry(id, fetch_geometry(reference).await))
                        .ok();
                    context.request_repaint();
                });
            }
        }

        fn priority_shards(&self) -> BTreeSet<String> {
            let mut shards = BTreeSet::new();
            if let Some(selected) = self.selected
                && let Some(shard) = self.atlas.unloaded_entity_shard(selected)
            {
                self.atlas.unloaded_with_dependencies(shard, &mut shards);
            }
            if let Some(overlay) = &self.overlay.source {
                for event in &overlay.package().events {
                    for id in event.anchors(&self.atlas, self.body) {
                        if let Some(shard) = self.atlas.unloaded_entity_shard(id) {
                            self.atlas.unloaded_with_dependencies(shard, &mut shards);
                        }
                    }
                }
            }
            shards
        }
    }

    async fn fetch_geometry(reference: GeometryReference) -> Result<GeometryBundle, String> {
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
}

#[cfg(target_arch = "wasm32")]
pub(super) use wasm::StreamingLoads;

#[cfg(test)]
mod tests;
