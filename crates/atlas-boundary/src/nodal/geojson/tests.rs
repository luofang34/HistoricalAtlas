use super::*;

#[test]
fn schematic_links_cross_the_antimeridian_without_a_global_chord() {
    let from = LonLat {
        longitude: 179.0,
        latitude: 10.0,
    };
    let to = LonLat {
        longitude: -179.0,
        latitude: 20.0,
    };
    assert_eq!(
        segments(from, to),
        vec![
            [[179.0, 10.0], [180.0, 15.0]],
            [[-180.0, 15.0], [-179.0, 20.0]],
        ]
    );
    assert_eq!(
        segments(to, from),
        vec![
            [[-179.0, 20.0], [-180.0, 15.0]],
            [[180.0, 15.0], [179.0, 10.0]],
        ]
    );
    let east = LonLat {
        longitude: 180.0,
        latitude: 10.0,
    };
    let west = LonLat {
        longitude: -180.0,
        latitude: 10.0,
    };
    assert!(segments(east, west).is_empty());
    assert!(segments(from, from).is_empty());
    assert_eq!(segments(east, to), vec![[[-180.0, 10.0], [-179.0, 20.0]]]);
}
