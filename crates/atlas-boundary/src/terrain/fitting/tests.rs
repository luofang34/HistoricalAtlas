use super::*;
use crate::terrain::{derive, tests::request};
use geo::{Area, BooleanOps, Contains, Point, unary_union};

#[test]
fn fitting_shrinks_and_expands_without_losing_seeds_coverage_or_determinism() {
    for area in [0.23, 0.70] {
        let mut input = request();
        input.areas = vec![AreaTarget {
            unit: 0,
            square_km: area,
            tolerance_square_km: 0.02,
        }];
        let result = derive(&input).unwrap();
        let polygons: Vec<_> = result
            .iter()
            .map(|r| crate::polygon::resolve(&r.vertices, &r.polygons).unwrap())
            .collect();
        assert!(input.areas[0].accepts(review::area_square_km(&polygons[0])));
        let envelope = crate::polygon::resolve(&input.vertices, &input.envelope).unwrap();
        assert!(polygons[0].intersection(&polygons[1]).unsigned_area() < 1e-16);
        assert!(unary_union(&polygons).xor(&envelope).unsigned_area() < 1e-16);
        for seed in &input.seeds {
            assert!(
                polygons[seed.unit]
                    .contains(&Point::new(seed.point.longitude, seed.point.latitude))
            );
        }
        input.seeds.reverse();
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            serde_json::to_value(derive(&input).unwrap()).unwrap()
        );
    }
}

#[test]
fn fitting_reports_an_unattainable_area_instead_of_reassigning_a_seed() {
    let mut input = request();
    input.areas = vec![AreaTarget {
        unit: 0,
        square_km: 0.001,
        tolerance_square_km: 0.0001,
    }];
    assert!(matches!(
        derive(&input),
        Err(TerrainError::AreaFit { unit: 0, .. })
    ));
    input.areas[0].square_km = 10.;
    assert!(matches!(
        derive(&input),
        Err(TerrainError::AreaFit { unit: 0, .. })
    ));
}

#[test]
fn malformed_and_duplicate_area_constraints_are_rejected() {
    let mut input = request();
    let target = AreaTarget {
        unit: 0,
        square_km: 0.23,
        tolerance_square_km: 0.02,
    };
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        input.areas = vec![AreaTarget {
            square_km: value,
            ..target.clone()
        }];
        assert!(matches!(derive(&input), Err(TerrainError::Input(_))));
        input.areas = vec![AreaTarget {
            tolerance_square_km: value,
            ..target.clone()
        }];
        assert!(matches!(derive(&input), Err(TerrainError::Input(_))));
    }
    input.areas = vec![target.clone(), target.clone()];
    assert!(derive(&input).is_err());
    input.areas = vec![AreaTarget {
        unit: 2,
        ..target.clone()
    }];
    assert!(derive(&input).is_err());
    input.areas = vec![AreaTarget {
        tolerance_square_km: 1.,
        ..target
    }];
    assert!(derive(&input).is_err());
}

#[test]
fn connected_seed_groups_survive_shrinking_and_target_order_does_not_change_the_fit() {
    let mut input = request();
    let mut seed = input.seeds[0].clone();
    for y in [150., 650.] {
        let point = crate::terrain::unproject(geo::Coord { x: 150., y });
        seed.point.longitude = point.x;
        seed.point.latitude = point.y;
        input.seeds.push(seed.clone());
    }
    input.areas = vec![
        AreaTarget {
            unit: 0,
            square_km: 0.08,
            tolerance_square_km: 0.012,
        },
        AreaTarget {
            unit: 1,
            square_km: 0.83,
            tolerance_square_km: 0.02,
        },
    ];
    let result = derive(&input).unwrap();
    assert_eq!(result[0].polygons.len(), 1);
    for target in &input.areas {
        let region = &result[target.unit];
        let polygon = crate::polygon::resolve(&region.vertices, &region.polygons).unwrap();
        assert!(target.accepts(review::area_square_km(&polygon)));
        for seed in input.seeds.iter().filter(|s| s.unit == target.unit) {
            assert!(polygon.contains(&Point::new(seed.point.longitude, seed.point.latitude)));
        }
    }
    input.areas.reverse();
    input.seeds.reverse();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(derive(&input).unwrap()).unwrap()
    );
}

#[test]
fn incompatible_area_constraints_fail_together() {
    let mut input = request();
    input.areas = (0..2)
        .map(|unit| AreaTarget {
            unit,
            square_km: 0.1,
            tolerance_square_km: 0.03,
        })
        .collect();
    assert!(matches!(derive(&input), Err(TerrainError::AreaFit { .. })));
}
