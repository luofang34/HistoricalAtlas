use geo::{
    Coord, Polygon,
    line_intersection::{LineIntersection, line_intersection},
};

use super::TopologyError;

// A cycle in the ring-contact graph disconnects the polygon's interior. Contact
// nodes distinguish three rings meeting once from three distinct pairwise contacts.
pub(super) fn check(polygon: &Polygon<f64>) -> Result<(), TopologyError> {
    let rings: Vec<_> = std::iter::once(polygon.exterior())
        .chain(polygon.interiors())
        .collect();
    let mut contacts: Vec<(Coord<f64>, Vec<usize>)> = Vec::new();
    for (left, ring) in rings.iter().enumerate() {
        for (right, other) in rings.iter().enumerate().skip(left + 1) {
            for a in ring.lines() {
                for b in other.lines() {
                    if let Some(LineIntersection::SinglePoint { intersection, .. }) =
                        line_intersection(a, b)
                    {
                        if let Some((_, members)) = contacts
                            .iter_mut()
                            .find(|(point, _)| *point == intersection)
                        {
                            for member in [left, right] {
                                if !members.contains(&member) {
                                    members.push(member);
                                }
                            }
                        } else {
                            contacts.push((intersection, vec![left, right]));
                        }
                    }
                }
            }
        }
    }
    let mut components: Vec<_> = (0..rings.len()).collect();
    for (_, members) in contacts {
        let mut joined = Vec::new();
        for member in members {
            let component = *components.get(member).ok_or(TopologyError::Index(member))?;
            if joined.contains(&component) {
                return Err(TopologyError::Disconnected);
            }
            joined.push(component);
        }
        if let Some(first) = joined.first().copied() {
            for component in &mut components {
                if joined.contains(component) {
                    *component = first;
                }
            }
        }
    }
    Ok(())
}
