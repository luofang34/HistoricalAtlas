use super::*;
use crate::terrain::{AreaTarget, TerrainSeed, derive, tests::request, unproject};
use atlas_geo::coordinates::LonLat;

#[test]
fn fitting_retains_a_cited_interface_and_the_paths_connecting_it_to_seeds() {
    let mut input = request();
    for (seed, x) in input.seeds.iter_mut().zip([350., 850.]) {
        let point = unproject(geo::Coord { x, y: 450. });
        seed.point = LonLat {
            longitude: point.x,
            latitude: point.y,
        };
    }
    let point = unproject(geo::Coord { x: 600., y: 150. });
    input.units = 3;
    input.seeds.push(TerrainSeed {
        unit: 2,
        point: LonLat {
            longitude: point.x,
            latitude: point.y,
        },
    });
    input.neighbors = vec![[0, 1]];
    input.areas = vec![AreaTarget {
        unit: 0,
        square_km: 0.12,
        tolerance_square_km: 0.012,
    }];
    let result = derive(&input).unwrap();
    assert_eq!(result[0].polygons.len(), 1);
    let polygon = crate::polygon::resolve(&result[0].vertices, &result[0].polygons).unwrap();
    let area = crate::terrain::review::area_square_km(&polygon);
    assert!((area - 0.12).abs() <= 0.012);
    validate_neighbors(&input, &result).unwrap();
    input.neighbors = vec![[1, 0], [0, 1]];
    input.seeds.reverse();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(derive(&input).unwrap()).unwrap()
    );
}

#[test]
fn unknown_self_and_seed_blocked_neighbor_pairs_are_rejected() {
    let mut input = request();
    input.areas = vec![AreaTarget {
        unit: 0,
        square_km: 0.47,
        tolerance_square_km: 0.01,
    }];
    for pair in [[0, 0], [0, 2]] {
        input.neighbors = vec![pair];
        assert!(matches!(derive(&input), Err(TerrainError::Input(_))));
    }
    input.units = 3;
    for row in 0..8 {
        let point = unproject(geo::Coord {
            x: 650.,
            y: 50. + 100. * f64::from(row),
        });
        input.seeds.push(TerrainSeed {
            unit: 2,
            point: LonLat {
                longitude: point.x,
                latitude: point.y,
            },
        });
    }
    input.neighbors = vec![[0, 1]];
    assert!(
        derive(&input)
            .unwrap_err()
            .to_string()
            .contains("cannot be connected without splitting a unit or moving a seed")
    );
}

#[test]
fn cited_neighbors_gain_an_edge_without_splitting_the_intervening_unit() {
    use geo::{Area, BooleanOps, Contains, Point, unary_union};
    let mut input = request();
    let point = unproject(geo::Coord { x: 650., y: 450. });
    input.units = 3;
    input.seeds.push(TerrainSeed {
        unit: 2,
        point: LonLat {
            longitude: point.x,
            latitude: point.y,
        },
    });
    let initial = derive(&input).unwrap();
    input.neighbors = vec![[0, 1], [0, 2], [1, 2]];
    assert!(validate_neighbors(&input, &initial).is_err());
    input.areas = vec![AreaTarget {
        unit: 0,
        square_km: 0.30,
        tolerance_square_km: 0.012,
    }];
    let result = derive(&input).unwrap();
    validate_neighbors(&input, &result).unwrap();
    let polygons: Vec<_> = result
        .iter()
        .map(|r| {
            assert_eq!(r.polygons.len(), 1);
            crate::polygon::resolve(&r.vertices, &r.polygons).unwrap()
        })
        .collect();
    let envelope = crate::polygon::resolve(&input.vertices, &input.envelope).unwrap();
    assert!(unary_union(&polygons).xor(&envelope).unsigned_area() < 1e-16);
    for (index, polygon) in polygons.iter().enumerate() {
        for other in polygons.iter().skip(index + 1) {
            assert!(polygon.intersection(other).unsigned_area() < 1e-16);
        }
    }
    for seed in &input.seeds {
        assert!(
            polygons[seed.unit].contains(&Point::new(seed.point.longitude, seed.point.latitude))
        );
    }
    assert!((crate::terrain::review::area_square_km(&polygons[0]) - 0.30).abs() <= 0.012);
    input.neighbors = vec![[2, 1], [2, 0], [1, 0], [0, 1]];
    input.seeds.reverse();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(derive(&input).unwrap()).unwrap()
    );
}
