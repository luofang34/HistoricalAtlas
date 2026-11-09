use super::{EntityId, GlobeCameraState, LatLon, Point2, sphere};
use rstar::{AABB, RTree, RTreeObject};

struct Segment {
    envelope: AABB<[f64; 3]>,
    data: (EntityId, LatLon, LatLon),
}

impl RTreeObject for Segment {
    type Envelope = AABB<[f64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

#[derive(Default)]
pub(super) struct LineIndex(RTree<Segment>);

impl LineIndex {
    pub(super) fn new(paths: Vec<(EntityId, Vec<Vec<[f64; 2]>>)>) -> Self {
        let mut segments = Vec::new();
        for (id, paths) in paths {
            for path in paths {
                for (a, b) in path.iter().zip(path.iter().skip(1)) {
                    let steps = (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).ceil().max(1.0) as u32;
                    let at = |i| {
                        let t = f64::from(i) / f64::from(steps);
                        LatLon::new(a[1] + (b[1] - a[1]) * t, a[0] + (b[0] - a[0]) * t)
                    };
                    for i in 0..steps {
                        let (a, b) = (at(i), at(i + 1));
                        let sa = sphere(a);
                        let sb = sphere(b);
                        // The margin encloses the curved arc between endpoints at one-degree sampling.
                        let lo = [
                            sa[0].min(sb[0]) - 0.001,
                            sa[1].min(sb[1]) - 0.001,
                            sa[2].min(sb[2]) - 0.001,
                        ];
                        let hi = [
                            sa[0].max(sb[0]) + 0.001,
                            sa[1].max(sb[1]) + 0.001,
                            sa[2].max(sb[2]) + 0.001,
                        ];
                        segments.push(Segment {
                            envelope: AABB::from_corners(lo, hi),
                            data: (id, a, b),
                        });
                    }
                }
            }
        }
        Self(RTree::bulk_load(segments))
    }

    pub(super) fn pick(
        &self,
        camera: &GlobeCameraState,
        pixel: Point2<f64>,
        anchor: [f64; 3],
        reach: f64,
    ) -> Vec<EntityId> {
        let radius = reach.sqrt();
        let envelope = AABB::from_corners(anchor.map(|v| v - radius), anchor.map(|v| v + radius));
        self.0
            .locate_in_envelope_intersecting(envelope)
            .filter_map(|segment| {
                let (id, a, b) = segment.data;
                if camera.is_location_occluded(a) || camera.is_location_occluded(b) {
                    return None;
                }
                let a = camera.location_to_screen(a, 0.0);
                let b = camera.location_to_screen(b, 0.0);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let square = dx * dx + dy * dy;
                let t = if square == 0.0 {
                    0.0
                } else {
                    ((pixel.x - a.x) * dx + (pixel.y - a.y) * dy) / square
                }
                .clamp(0.0, 1.0);
                ((pixel.x - a.x - t * dx).hypot(pixel.y - a.y - t * dy) <= 12.0).then_some(id)
            })
            .collect()
    }
}
