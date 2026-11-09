use super::*;

#[test]
fn heights_interpolate_between_cell_centres_and_stop_at_the_grid_edge() {
    let grid = ElevationGrid {
        width: 2,
        height: 2,
        west_m: 0.0,
        north_m: 200.0,
        cell_m: 100.0,
        elevations_m: vec![0, 100, 200, 300],
    };
    let at = |x: f64, y: f64| {
        let point = super::super::unproject(geo::Coord { x, y });
        LonLat {
            longitude: point.x,
            latitude: point.y,
        }
    };
    let centre = grid.height_m(at(100.0, 100.0)).unwrap();
    assert!((centre - 150.0).abs() < 1e-6, "{centre}");
    let north_west = grid.height_m(at(50.0, 150.0)).unwrap();
    assert!(north_west.abs() < 1e-6);
    assert!(grid.height_m(at(10.0, 100.0)).is_none());
    assert!(grid.height_m(at(100.0, 199.0)).is_none());
}
