use super::*;
use geo::MapCoords;

pub(super) fn tiles(
    shape: &Shape,
    bounds: Rect,
    zoom: u8,
) -> Result<Vec<(Address, Shape)>, TileError> {
    let side = f64::from(1_u32 << zoom);
    let buffer = 64.0 / f64::from(EXTENT);
    let left = (bounds.min().x * side - buffer).floor() as i64;
    let right = (bounds.max().x * side + buffer).floor() as i64;
    let top = ((bounds.min().y * side - buffer).floor() as i64).max(0);
    let bottom = ((bounds.max().y * side + buffer).floor() as i64).min(side as i64 - 1);
    let count = (right - left + 1).saturating_mul(bottom - top + 1);
    if count > 100_000 {
        return Err(TileError::Coverage { zoom, count });
    }
    let mut result = Vec::new();
    for x in left..=right {
        for y in top..=bottom {
            let bounds = Rect::new(
                Coord {
                    x: (x as f64 - buffer) / side,
                    y: (y as f64 - buffer) / side,
                },
                Coord {
                    x: (x as f64 + 1.0 + buffer) / side,
                    y: (y as f64 + 1.0 + buffer) / side,
                },
            );
            let local = |p: Coord| Coord {
                x: (p.x * side - x as f64) * f64::from(EXTENT),
                y: (p.y * side - y as f64) * f64::from(EXTENT),
            };
            let clipped = match shape.clip(bounds) {
                Shape::Lines(lines) => Shape::Lines(lines.map_coords(local)),
                Shape::Polygons(polygons) => Shape::Polygons(polygons.map_coords(local)),
                Shape::Point(point) => Shape::Point(point),
            };
            result.push((
                Address {
                    z: zoom,
                    x: x.rem_euclid(side as i64) as u32,
                    y: y as u32,
                },
                clipped,
            ));
        }
    }
    Ok(result)
}
