use super::{Overlay, TimeError};
use crate::{catalog::Atlas, selection::SelectedTime};
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use serde_json::{Value, json};

impl Overlay {
    /// Build event markers at reviewed atlas anchors with their source precision.
    pub fn features(
        &self,
        atlas: &Atlas,
        body: BodyId,
        time: &SelectedTime,
        selected: Option<EntityId>,
    ) -> Result<Vec<Value>, TimeError> {
        let mut features = Vec::new();
        for event in &self.package.events {
            if !event.matches(time.at, time.selection)? {
                continue;
            }
            for original in &event.entities {
                let Some(id) = atlas.resolve(*original) else {
                    continue;
                };
                let Some(entry) = atlas.entry(id).filter(|entry| entry.entity.body == body) else {
                    continue;
                };
                for record in &entry.records {
                    let Some(coordinate) = &record.observation.coordinate else {
                        continue;
                    };
                    features.push(json!({
                        "type": "Feature",
                        "geometry": {"type": "Point", "coordinates":
                            [coordinate.point.longitude, coordinate.point.latitude]},
                        "properties": {
                            "geometry_role": "external_event", "event_id": event.id,
                            "event_title": event.title, "package_title": self.package.title,
                            "package_origin": self.origin, "entity_id": id, "anchor_id": original,
                            "record_id": record.id, "selected": selected == Some(id),
                            "datum": coordinate.datum,
                            "resolution_arcminutes": coordinate.resolution_arcminutes,
                            "source_id": record.source_id, "page_or_sheet": record.page_or_sheet,
                            "scan_page": record.scan_page, "validity": event.validity
                        }
                    }));
                }
            }
        }
        Ok(features)
    }
}
