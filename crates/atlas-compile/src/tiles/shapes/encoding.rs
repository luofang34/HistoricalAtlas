use super::Shape;
use geo::{Area, LineString, Orient, Polygon, orient::Direction};
use geozero::mvt::tile::GeomType;

pub(in crate::tiles) fn commands(shape: &Shape) -> (GeomType, Vec<u32>) {
    let mut output = Vec::new();
    let mut cursor = (0, 0);
    let kind = match shape {
        Shape::Point((x, y)) => {
            output.extend([9, zigzag(x.round() as i32), zigzag(y.round() as i32)]);
            GeomType::Point
        }
        Shape::Lines(lines) => {
            for line in &lines.0 {
                path(line, false, &mut cursor, &mut output);
            }
            GeomType::Linestring
        }
        Shape::Polygons(polygons) => {
            for polygon in &polygons.0 {
                let exterior = quantize(polygon.exterior());
                if exterior.0.len() < 4
                    || Polygon::new(exterior.clone(), vec![]).unsigned_area() < 0.5
                {
                    continue;
                }
                let holes = polygon
                    .interiors()
                    .iter()
                    .map(quantize)
                    .filter(|ring| {
                        ring.0.len() >= 4
                            && Polygon::new(ring.clone(), vec![]).unsigned_area() >= 0.5
                    })
                    .collect();
                // MVT's downward y axis requires positive exterior and negative interior area.
                let polygon = Polygon::new(exterior, holes).orient(Direction::Default);
                for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
                    path(ring, true, &mut cursor, &mut output);
                }
            }
            GeomType::Polygon
        }
    };
    (kind, output)
}

fn quantize(line: &LineString) -> LineString {
    let mut points: Vec<_> = line
        .0
        .iter()
        .map(|p| geo::Coord {
            x: p.x.round(),
            y: p.y.round(),
        })
        .collect();
    points.dedup();
    LineString(points)
}

fn path(line: &LineString, closed: bool, cursor: &mut (i32, i32), output: &mut Vec<u32>) {
    let line = quantize(line);
    let mut points = line.0;
    if closed && points.first() == points.last() {
        points.pop();
    }
    if points.len() < if closed { 3 } else { 2 } {
        return;
    }
    output.push(9);
    for (index, point) in points.iter().enumerate() {
        if index == 1 {
            output.push(((points.len() - 1) as u32) << 3 | 2);
        }
        let next = (point.x as i32, point.y as i32);
        output.extend([zigzag(next.0 - cursor.0), zigzag(next.1 - cursor.1)]);
        *cursor = next;
    }
    if closed {
        output.push(15);
    }
}

fn zigzag(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}
