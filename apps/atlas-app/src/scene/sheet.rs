//! Source-sheet locations keep coincident seats selectable without geographical inference.

use crate::catalog::Atlas;
use atlas_boundary::sheet::SheetTrace;
use atlas_core::identity::EntityId;
use atlas_geo::registration::PlanarPoint;

/// Coincident source symbols with their distinct geographical identities.
pub struct SheetPoint {
    /// Full-page raster position.
    pub point: PlanarPoint,
    /// Independently selectable identities sharing the symbol.
    pub entities: Vec<EntityId>,
    /// Source labels, in drawing order.
    pub names: Vec<String>,
    /// Whether the symbol includes an administrative seat.
    pub administrative: bool,
}

/// Group source-sheet symbols without merging their entity identities.
pub fn points(atlas: &Atlas, drawing: &SheetTrace) -> Vec<SheetPoint> {
    let mut points: Vec<SheetPoint> = Vec::new();
    for place in &drawing.places {
        if let Some(record) = atlas.data.records.iter().find(|r| r.id == place.record_id) {
            let administrative = record.observation.rank.is_some();
            if let Some(point) = points.iter_mut().find(|p| p.point == place.point) {
                point.entities.push(record.entity_id);
                point.names.push(record.observation.name.clone());
                point.administrative |= administrative;
            } else {
                points.push(SheetPoint {
                    point: place.point,
                    entities: vec![record.entity_id],
                    names: vec![record.observation.name.clone()],
                    administrative,
                });
            }
        }
    }
    points
}

/// Pick every identity at a symbol within an eight-point screen radius.
pub fn pick(points: &[SheetPoint], distance: impl Fn(PlanarPoint) -> f32) -> Vec<EntityId> {
    points
        .iter()
        .filter_map(|p| {
            let distance = distance(p.point);
            (distance <= 8.0).then_some((distance, p))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or_else(Vec::new, |(_, p)| p.entities.clone())
}

#[cfg(test)]
mod tests;
