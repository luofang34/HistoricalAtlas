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
fn unknown_self_and_initially_separated_neighbor_pairs_are_rejected() {
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
    let point = unproject(geo::Coord { x: 600., y: 450. });
    input.units = 3;
    input.seeds.push(TerrainSeed {
        unit: 2,
        point: LonLat {
            longitude: point.x,
            latitude: point.y,
        },
    });
    input.neighbors = vec![[0, 1]];
    assert!(
        derive(&input)
            .unwrap_err()
            .to_string()
            .contains("no initial shared edge")
    );
}
