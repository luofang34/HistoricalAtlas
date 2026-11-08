use super::*;
use geo::{Area, BooleanOps, Contains, Point, unary_union};

#[cfg(test)]
fn request() -> TerrainRequest {
    let grid = ElevationGrid {
        width: 12,
        height: 8,
        west_m: 0.,
        north_m: 800.,
        cell_m: 100.,
        elevations_m: vec![0; 96],
    };
    let point = |x, y| unproject(geo::Coord { x, y });
    let seed = |unit, x, y| {
        let p = point(x, y);
        TerrainSeed {
            unit,
            point: LonLat {
                longitude: p.x,
                latitude: p.y,
            },
        }
    };
    TerrainRequest {
        grid,
        vertices: vec![
            point(10., 10.),
            point(1190., 10.),
            point(1190., 790.),
            point(10., 790.),
        ],
        envelope: vec![IndexedPolygon {
            exterior: vec![0, 1, 2, 3, 0],
            interiors: vec![],
        }],
        seeds: vec![seed(0, 150., 450.), seed(1, 1050., 450.)],
        units: 2,
        slope_weight: 20.,
    }
}

#[test]
fn flat_partition_covers_the_envelope_without_overlapping_units() {
    let input = request();
    let result = derive(&input).unwrap();
    assert_eq!(result.len(), 2);
    let polygons: Vec<_> = result
        .iter()
        .map(|r| resolve(&r.vertices, &r.polygons).unwrap())
        .collect();
    let envelope = resolve(&input.vertices, &input.envelope).unwrap();
    assert!(polygons[0].intersection(&polygons[1]).unsigned_area() < 1e-16);
    assert!(unary_union(&polygons).xor(&envelope).unsigned_area() < 1e-16);
    for (unit, seed) in input.seeds.iter().enumerate() {
        assert!(polygons[unit].contains(&Point::new(seed.point.longitude, seed.point.latitude)));
    }
    let reverse = TerrainRequest {
        seeds: input.seeds.iter().rev().cloned().collect(),
        ..input
    };
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(derive(&reverse).unwrap()).unwrap()
    );
}

#[test]
fn a_steep_step_changes_the_partition_and_zero_weight_restores_distance_only() {
    let mut input = request();
    let flat = derive(&input).unwrap();
    for (i, z) in input.grid.elevations_m.iter_mut().enumerate() {
        if i % 12 >= 3 {
            *z = 1000;
        }
    }
    let terrain = derive(&input).unwrap();
    assert!(terrain[0].cells < flat[0].cells);
    let p = unproject(geo::Coord { x: 450., y: 450. });
    assert!(
        resolve(&flat[0].vertices, &flat[0].polygons)
            .unwrap()
            .contains(&Point::new(p.x, p.y))
    );
    assert!(
        resolve(&terrain[1].vertices, &terrain[1].polygons)
            .unwrap()
            .contains(&Point::new(p.x, p.y))
    );
    input.slope_weight = 0.;
    assert_eq!(
        serde_json::to_value(flat).unwrap(),
        serde_json::to_value(derive(&input).unwrap()).unwrap()
    );
}

#[test]
fn conflicting_seeds_voids_and_unseeded_components_are_rejected() {
    let mut input = request();
    input.seeds[1].point = input.seeds[0].point;
    assert!(matches!(derive(&input), Err(TerrainError::Conflict { .. })));
    let mut input = request();
    input.grid.elevations_m[0] = i16::MIN;
    assert!(derive(&input).is_err());
    let mut input = request();
    input.units = 3;
    assert!(derive(&input).is_err());
    let mut input = request();
    input.grid.west_m = 50.;
    assert!(derive(&input).is_err());
    let mut input = request();
    input.seeds[1].point.longitude = 10.;
    assert!(matches!(derive(&input), Err(TerrainError::Seed(1))));
    let mut input = request();
    input.vertices = vec![
        (10., 10.),
        (1190., 10.),
        (1190., 590.),
        (10., 590.),
        (10., 710.),
        (1190., 710.),
        (1190., 790.),
        (10., 790.),
    ]
    .into_iter()
    .map(|(x, y)| unproject(geo::Coord { x, y }))
    .collect();
    input.envelope.push(IndexedPolygon {
        exterior: vec![4, 5, 6, 7, 4],
        interiors: vec![],
    });
    assert!(
        derive(&input)
            .unwrap_err()
            .to_string()
            .contains("component needs a seed")
    );
}
