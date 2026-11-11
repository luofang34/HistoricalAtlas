//! Rendered-feature picking through MapLibre's cross-projection and terrain-aware query index.

use std::collections::BTreeSet;

use atlas_core::identity::EntityId;
use maplibre::{
    query::QueriedFeature,
    sdf::query::{QueryGeometry, QueryOptions},
};

use super::{MapRenderer, RenderError};

const POINT_AND_LINE_LAYERS: &[&str] = &[
    "selection",
    "external-events",
    "mapped-places",
    "observations",
    "crustal-pins",
    "territory-nodes",
    "line-selection",
    "registered-lines",
];

const BOUNDARY_LINE_LAYERS: &[&str] = &[
    "boundary-selection",
    "derived-selection",
    "boundary-lines",
    "possible-boundary-lines",
    "derived-lines",
];

const FILL_LAYERS: &[&str] = &["boundaries", "derived-boundaries"];

fn layer_options(names: &[&str]) -> QueryOptions {
    QueryOptions {
        layers: Some(names.iter().map(|name| (*name).to_owned()).collect()),
        filter: None,
    }
}

fn padded_box([x, y]: [f64; 2], [width, height]: [f64; 2], pad: f64) -> QueryGeometry {
    QueryGeometry::Box {
        min: [(x - pad).max(0.0), (y - pad).max(0.0)],
        max: [(x + pad).min(width), (y + pad).min(height)],
    }
}

fn parse_entities(features: Vec<QueriedFeature>) -> Vec<EntityId> {
    features
        .into_iter()
        .filter_map(|feature| {
            let value = feature.properties.get("entity_id")?;
            serde_json::from_value(value.clone()).ok()
        })
        .collect()
}

impl MapRenderer {
    pub(super) fn pick_rendered(&self, [x, y]: [f64; 2]) -> Result<Vec<EntityId>, RenderError> {
        let [width, height] = self.view_size();
        if !x.is_finite()
            || !y.is_finite()
            || width <= 0.0
            || height <= 0.0
            || x < 0.0
            || y < 0.0
            || x > width
            || y > height
        {
            return Ok(Vec::new());
        }
        let point_line_opts = layer_options(POINT_AND_LINE_LAYERS);
        let exact_points = parse_entities(
            self.map
                .query_rendered_features(QueryGeometry::Point([x, y]), &point_line_opts)?,
        );
        let near_points =
            parse_entities(self.map.query_rendered_features(
                padded_box([x, y], [width, height], 5.0),
                &point_line_opts,
            )?);
        let mut fills =
            parse_entities(self.map.query_rendered_features(
                QueryGeometry::Point([x, y]),
                &layer_options(FILL_LAYERS),
            )?);
        let by_area = |a: &EntityId, b: &EntityId| {
            self.region_area(*a)
                .total_cmp(&self.region_area(*b))
                .then_with(|| a.cmp(b))
        };
        fills.sort_by(by_area);
        let mut boundary_strokes = parse_entities(self.map.query_rendered_features(
            padded_box([x, y], [width, height], 3.0),
            &layer_options(BOUNDARY_LINE_LAYERS),
        )?);
        boundary_strokes.sort_by(by_area);

        let mut seen = BTreeSet::new();
        let hits: Vec<EntityId> = exact_points
            .into_iter()
            .chain(near_points)
            .chain(fills)
            .chain(boundary_strokes)
            .filter(|id| seen.insert(*id))
            .collect();
        if !hits.is_empty() || !self.map.needs_redraw() {
            return Ok(hits);
        }
        self.fallback_pick([x, y])
    }

    fn region_area(&self, id: EntityId) -> f64 {
        self.picking
            .region_area(id)
            .min(self.selection_picking.region_area(id))
    }

    fn fallback_pick(&self, [x, y]: [f64; 2]) -> Result<Vec<EntityId>, RenderError> {
        let camera = maplibre::render::projection::globe_camera_for_view(self.map.view_state())?;
        let pixel = cgmath::Point2::new(x, y);
        let mut seen = BTreeSet::new();
        Ok(self
            .selection_picking
            .pick(&camera, pixel)
            .into_iter()
            .chain(self.picking.pick(&camera, pixel))
            .filter(|id| seen.insert(*id))
            .collect())
    }
}
