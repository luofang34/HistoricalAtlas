use std::collections::{BTreeMap, BTreeSet};

use atlas_core::identity::EntityId;
use cgmath::Point2;
use geo::{Area, BoundingRect, Intersects, LineString, MultiPolygon, Point, Polygon};
use maplibre::{coords::LatLon, projection::globe::camera::GlobeCameraState};
use rstar::{AABB, RTree, RTreeObject, primitives::GeomWithData};
use serde::Deserialize;
use serde_json::{Value, json};

mod lines;

type Located = GeomWithData<[f64; 3], (EntityId, LatLon)>;

struct Region {
    id: EntityId,
    shape: MultiPolygon,
    area: f64,
    envelope: AABB<[f64; 2]>,
}

impl RTreeObject for Region {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "coordinates")]
enum Geometry {
    Point([f64; 2]),
    LineString(Vec<[f64; 2]>),
    MultiLineString(Vec<Vec<[f64; 2]>>),
    Polygon(Vec<Vec<[f64; 2]>>),
    MultiPolygon(Vec<Vec<Vec<[f64; 2]>>>),
    #[serde(other)]
    Other,
}

/// Spatial and identity indexes are rebuilt only when the visible source changes.
#[derive(Default)]
pub(super) struct Picking {
    points: RTree<Located>,
    lines: lines::LineIndex,
    regions: RTree<Region>,
    region_areas: BTreeMap<EntityId, f64>,
    entities: BTreeMap<EntityId, Vec<Value>>,
}

impl Picking {
    pub(super) fn new(collection: &Value) -> Result<Self, serde_json::Error> {
        let mut index = Self::default();
        let mut points = Vec::new();
        let mut regions = Vec::new();
        let mut lines = Vec::new();
        for feature in collection
            .get("features")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(id) = feature.pointer("/properties/entity_id") else {
                continue;
            };
            let id = serde_json::from_value(id.clone())?;
            index.entities.entry(id).or_default().push(feature.clone());
            match serde_json::from_value::<Geometry>(
                feature.get("geometry").cloned().unwrap_or_default(),
            )? {
                Geometry::Point([longitude, latitude]) => {
                    let point = LatLon::new(latitude, longitude);
                    points.push(GeomWithData::new(sphere(point), (id, point)));
                }
                Geometry::Polygon(rings) => {
                    if let Some(built) = region(id, vec![rings]) {
                        *index.region_areas.entry(id).or_insert(0.0) += built.area;
                        regions.push(built);
                    }
                }
                Geometry::MultiPolygon(polygons) => {
                    if let Some(built) = region(id, polygons) {
                        *index.region_areas.entry(id).or_insert(0.0) += built.area;
                        regions.push(built);
                    }
                }
                Geometry::LineString(path) => lines.push((id, vec![path])),
                Geometry::MultiLineString(paths) => lines.push((id, paths)),
                Geometry::Other => {}
            }
        }
        index.points = RTree::bulk_load(points);
        index.lines = lines::LineIndex::new(lines);
        index.regions = RTree::bulk_load(regions);
        Ok(index)
    }

    pub(super) fn region_area(&self, id: EntityId) -> f64 {
        self.region_areas.get(&id).copied().unwrap_or(f64::INFINITY)
    }

    pub(super) fn selection(&self, id: Option<EntityId>, mut extra: Vec<Value>) -> Value {
        if let Some(features) = id.and_then(|id| self.entities.get(&id)) {
            extra.extend(features.iter().cloned().map(|mut feature| {
                if let Some(properties) =
                    feature.get_mut("properties").and_then(Value::as_object_mut)
                {
                    properties.insert("selected".into(), true.into());
                }
                feature
            }));
        }
        json!({"type": "FeatureCollection", "features": extra})
    }

    pub(super) fn pick(&self, camera: &GlobeCameraState, pixel: Point2<f64>) -> Vec<EntityId> {
        if !camera.is_point_on_map_surface(pixel) {
            return Vec::new();
        }
        let Some(ground) = camera.screen_point_to_location(pixel) else {
            return Vec::new();
        };
        let anchor = sphere(ground);
        // Sample the hit box on the sphere so polar and antimeridian queries use the same index.
        let reach = [-12.0, 0.0, 12.0]
            .into_iter()
            .flat_map(|x| {
                [-12.0, 0.0, 12.0].into_iter().filter_map(move |y| {
                    camera.screen_point_to_location(Point2::new(pixel.x + x, pixel.y + y))
                })
            })
            .map(|p| distance_squared(anchor, sphere(p)))
            .fold(0.0, f64::max);
        let mut point_hits = Vec::new();
        for point in self.points.locate_within_distance(anchor, reach) {
            if camera.is_location_occluded(point.data.1) {
                continue;
            }
            let screen = camera.location_to_screen(point.data.1, 0.0);
            let dist = (screen.x - pixel.x).hypot(screen.y - pixel.y);
            if dist <= 12.0 {
                point_hits.push((dist, point.data.0));
            }
        }
        point_hits.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let line_hits = self.lines.pick(camera, pixel, anchor, reach);
        let point = Point::new(ground.longitude, ground.latitude);
        let envelope = AABB::from_point([ground.longitude, ground.latitude]);
        let mut region_hits = Vec::new();
        for region in self.regions.locate_in_envelope_intersecting(envelope) {
            if region.shape.intersects(&point) {
                region_hits.push((region.area, region.id));
            }
        }
        region_hits.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let mut seen = BTreeSet::new();
        point_hits
            .into_iter()
            .map(|(_, id)| id)
            .chain(line_hits)
            .chain(region_hits.into_iter().map(|(_, id)| id))
            .filter(|id| seen.insert(*id))
            .collect()
    }
}

fn region(id: EntityId, components: Vec<Vec<Vec<[f64; 2]>>>) -> Option<Region> {
    let shape = MultiPolygon(
        components
            .into_iter()
            .filter_map(|rings| {
                let mut rings = rings.into_iter().map(|ring| {
                    LineString::from(ring.into_iter().map(|[x, y]| (x, y)).collect::<Vec<_>>())
                });
                Some(Polygon::new(rings.next()?, rings.collect()))
            })
            .collect(),
    );
    let bounds = shape.bounding_rect()?;
    let area = shape.unsigned_area();
    Some(Region {
        id,
        shape,
        area,
        envelope: AABB::from_corners(
            [bounds.min().x, bounds.min().y],
            [bounds.max().x, bounds.max().y],
        ),
    })
}

fn sphere(point: LatLon) -> [f64; 3] {
    let latitude = point.latitude.to_radians();
    let longitude = point.longitude.to_radians();
    [
        latitude.cos() * longitude.cos(),
        latitude.cos() * longitude.sin(),
        latitude.sin(),
    ]
}

fn distance_squared(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum()
}

#[cfg(test)]
mod tests;
