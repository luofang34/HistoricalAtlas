use super::*;

#[cfg(test)]
fn points(values: &[(f64, f64)]) -> Vec<PlanarPoint> {
    values
        .iter()
        .map(|(x, y)| PlanarPoint { x: *x, y: *y })
        .collect()
}

#[cfg(test)]
fn polygon(exterior: &[usize], interiors: &[&[usize]]) -> IndexedPolygon {
    IndexedPolygon {
        exterior: exterior.to_vec(),
        interiors: interiors.iter().map(|ring| ring.to_vec()).collect(),
    }
}

#[test]
fn rejects_bowties_spikes_unclosed_and_out_of_range_rings() {
    let vertices = points(&[(0., 0.), (4., 0.), (4., 4.), (0., 4.), (2., 6.)]);
    for exterior in [
        vec![0, 2, 1, 3, 0],
        vec![0, 1, 2, 4, 2, 3, 0],
        vec![0, 1, 2, 3],
        vec![0, 1, 9, 0],
        vec![0, 1, 1, 2, 0],
    ] {
        assert!(resolve(&vertices, &[polygon(&exterior, &[])]).is_err());
    }
    let mut invalid = vertices.clone();
    invalid[1].x = f64::NAN;
    assert!(matches!(
        resolve(&invalid, &[polygon(&[0, 1, 2, 3, 0], &[])]),
        Err(TopologyError::Coordinate(1))
    ));
}

#[test]
fn holes_are_contained_and_multipolygon_components_do_not_overlap() {
    let vertices = points(&[
        (0., 0.),
        (10., 0.),
        (10., 10.),
        (0., 10.),
        (2., 2.),
        (4., 2.),
        (4., 4.),
        (2., 4.),
        (20., 0.),
        (22., 0.),
        (22., 2.),
        (20., 2.),
    ]);
    let exterior = polygon(&[0, 1, 2, 3, 0], &[&[4, 5, 6, 7, 4]]);
    let island = polygon(&[8, 9, 10, 11, 8], &[]);
    let geometry = resolve(&vertices, &[exterior.clone(), island]).unwrap();
    assert_eq!(geometry.unsigned_area(), 100.);
    assert!(
        resolve(
            &vertices,
            &[polygon(&[0, 1, 2, 3, 0], &[&[8, 9, 10, 11, 8]])]
        )
        .is_err()
    );
    assert!(resolve(&vertices, &[exterior.clone(), exterior]).is_err());
    let nested_holes = polygon(&[0, 1, 2, 3, 0], &[&[4, 5, 6, 7, 4], &[4, 5, 6, 7, 4]]);
    assert!(resolve(&vertices, &[nested_holes]).is_err());
}

#[test]
fn ring_contacts_cannot_disconnect_the_polygon_interior() {
    let vertices = points(&[
        (0., 0.),
        (4., 0.),
        (4., 4.),
        (0., 4.),
        (0., 2.),
        (2., 1.),
        (4., 2.),
        (2., 3.),
    ]);
    let disconnected = polygon(&[0, 1, 2, 3, 0], &[&[4, 5, 6, 7, 4]]);
    assert!(matches!(
        resolve(&vertices, &[disconnected]),
        Err(TopologyError::Disconnected)
    ));
    let touching_once = polygon(&[0, 1, 2, 3, 0], &[&[4, 5, 7, 4]]);
    assert!(resolve(&vertices, &[touching_once]).is_ok());
}

#[test]
fn sampling_reuses_shared_edges_in_reverse_and_enforces_budget() {
    let vertices = points(&[(0., 0.), (4., 0.), (4., 4.), (0., 4.), (8., 0.), (8., 4.)]);
    let left = polygon(&[0, 1, 2, 3, 0], &[]);
    let right = polygon(&[1, 4, 5, 2, 1], &[]);
    let (points, polygons) = densify(&vertices, &[left.clone(), right], 1.).unwrap();
    assert_eq!(points.len(), 27);
    let a = &polygons[0].exterior;
    let b = &polygons[1].exterior;
    let shared: Vec<_> = a
        .iter()
        .copied()
        .filter(|index| points[*index].x == 4.)
        .collect();
    assert_eq!(shared.len(), 5);
    assert!(shared.iter().all(|index| b.contains(index)));
    for polygon in polygons {
        let geom = resolve(&points, &[polygon]).unwrap();
        for line in geom.0[0].exterior().lines() {
            assert!((line.start.x - line.end.x).hypot(line.start.y - line.end.y) <= 1.);
        }
    }
    assert!(densify(&vertices, std::slice::from_ref(&left), 0.).is_err());
    assert!(densify(&vertices, &[left], 1e-15).is_err());
}
