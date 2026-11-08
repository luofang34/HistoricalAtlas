use super::{IndexedPolygon, TopologyError};
use atlas_geo::registration::PlanarPoint;
use std::collections::BTreeMap;

/// Subdivide source edges before nonlinear registration, reusing shared edges.
///
/// Returns the expanded vertex array and rings whose source segments are no
/// longer than `maximum_length`. Original vertex indices retain their meaning.
pub fn densify(
    vertices: &[PlanarPoint],
    polygons: &[IndexedPolygon],
    maximum_length: f64,
) -> Result<(Vec<PlanarPoint>, Vec<IndexedPolygon>), TopologyError> {
    for polygon in polygons {
        super::resolve(vertices, std::slice::from_ref(polygon))?;
    }
    if !maximum_length.is_finite() || maximum_length <= 0.0 || vertices.len() > 1_000_000 {
        return Err(TopologyError::Sampling);
    }
    let mut points = vertices.to_vec();
    let mut edges = BTreeMap::new();
    let mut result = Vec::new();
    for polygon in polygons {
        let mut expand =
            |ring: &[usize]| expand_ring(ring, &mut points, &mut edges, maximum_length);
        result.push(IndexedPolygon {
            exterior: expand(&polygon.exterior)?,
            interiors: polygon
                .interiors
                .iter()
                .map(|ring| expand(ring))
                .collect::<Result<_, _>>()?,
        });
    }
    Ok((points, result))
}

fn expand_ring(
    ring: &[usize],
    points: &mut Vec<PlanarPoint>,
    edges: &mut BTreeMap<(usize, usize), Vec<usize>>,
    spacing: f64,
) -> Result<Vec<usize>, TopologyError> {
    let mut result = Vec::new();
    for pair in ring.windows(2) {
        let [a, b] = pair else {
            return Err(TopologyError::Ring);
        };
        let key = ((*a).min(*b), (*a).max(*b));
        let indices = match edges.get(&key) {
            Some(indices) => indices.clone(),
            None => {
                let from = *points.get(key.0).ok_or(TopologyError::Index(key.0))?;
                let to = *points.get(key.1).ok_or(TopologyError::Index(key.1))?;
                let count = (from.x - to.x).hypot(from.y - to.y) / spacing;
                if !count.is_finite() || count.ceil() + points.len() as f64 > 1_000_000.0 {
                    return Err(TopologyError::Sampling);
                }
                let segments = (count.ceil() as usize).max(1);
                let mut indices = vec![key.0];
                for step in 1..segments {
                    let fraction = step as f64 / segments as f64;
                    indices.push(points.len());
                    points.push(PlanarPoint {
                        x: from.x + fraction * (to.x - from.x),
                        y: from.y + fraction * (to.y - from.y),
                    });
                }
                indices.push(key.1);
                edges.insert(key, indices.clone());
                indices
            }
        };
        let mut ordered = indices;
        if *a > *b {
            ordered.reverse();
        }
        result.extend(ordered.into_iter().skip(1));
    }
    if let Some(last) = result.last().copied() {
        result.insert(0, last);
    }
    Ok(result)
}
