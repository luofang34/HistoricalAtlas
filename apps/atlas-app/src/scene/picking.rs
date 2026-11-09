use std::collections::{BTreeMap, BTreeSet};

use atlas_core::identity::EntityId;
use cgmath::Point2;
use geo::{BoundingRect, Intersects, LineString, MultiPolygon, Point, Polygon};
use maplibre::{coords::LatLon, projection::globe::camera::GlobeCameraState};
use rstar::{AABB, RTree, RTreeObject, primitives::GeomWithData};
use serde::Deserialize;
use serde_json::{Value, json};

type Located = GeomWithData<[f64; 3], (EntityId, LatLon)>;

struct Region {
    id: EntityId,
    shape: MultiPolygon,
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
    Polygon(Vec<Vec<[f64; 2]>>),
    MultiPolygon(Vec<Vec<Vec<[f64; 2]>>>),
    #[serde(other)]
    Other,
}

/// Spatial and identity indexes are rebuilt only when the visible source changes.
#[derive(Default)]
pub(super) struct Picking {
    points: RTree<Located>,
    regions: RTree<Region>,
    entities: BTreeMap<EntityId, Vec<Value>>,
}

impl Picking {
    pub(super) fn new(collection: &Value) -> Result<Self, serde_json::Error> {
        let mut index = Self::default();
        let mut points = Vec::new();
        let mut regions = Vec::new();
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
                Geometry::Polygon(rings) => regions.extend(region(id, vec![rings])),
                Geometry::MultiPolygon(polygons) => regions.extend(region(id, polygons)),
                Geometry::Other => {}
            }
        }
        index.points = RTree::bulk_load(points);
        index.regions = RTree::bulk_load(regions);
        Ok(index)
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
        let mut hits = BTreeSet::new();
        for point in self.points.locate_within_distance(anchor, reach) {
            if camera.is_location_occluded(point.data.1) {
                continue;
            }
            let screen = camera.location_to_screen(point.data.1, 0.0);
            if (screen.x - pixel.x).hypot(screen.y - pixel.y) <= 12.0 {
                hits.insert(point.data.0);
            }
        }
        let point = Point::new(ground.longitude, ground.latitude);
        let envelope = AABB::from_point([ground.longitude, ground.latitude]);
        for region in self.regions.locate_in_envelope_intersecting(envelope) {
            if region.shape.intersects(&point) {
                hits.insert(region.id);
            }
        }
        hits.into_iter().collect()
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
    Some(Region {
        id,
        shape,
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
