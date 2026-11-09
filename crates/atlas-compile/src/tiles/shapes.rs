use super::{
    TileError,
    projection::{Address, EXTENT},
};
use atlas_core::identity::FactId;
use atlas_geo::coordinates::LonLat;
use geo::{
    BooleanOps, BoundingRect, Coord, LineString, MultiLineString, MultiPolygon, Polygon, Rect,
};
use serde::Deserialize;

mod encoding;
mod placement;
pub(super) use encoding::commands;

#[derive(Deserialize)]
#[serde(tag = "type", content = "coordinates")]
enum Coordinates {
    Point([f64; 2]),
    LineString(Vec<[f64; 2]>),
    MultiLineString(Vec<Vec<[f64; 2]>>),
    Polygon(Vec<Vec<[f64; 2]>>),
    MultiPolygon(Vec<Vec<Vec<[f64; 2]>>>),
}

pub(super) enum Shape {
    Point((f64, f64)),
    Lines(MultiLineString),
    Polygons(MultiPolygon),
}

impl Shape {
    pub(super) fn from_json(value: serde_json::Value, fact: FactId) -> Result<Self, TileError> {
        Ok(match serde_json::from_value::<Coordinates>(value)? {
            Coordinates::Point([longitude, latitude]) => Self::Point(super::projection::project(
                LonLat {
                    longitude,
                    latitude,
                },
                fact,
            )?),
            Coordinates::LineString(line) => {
                Self::Lines(MultiLineString(vec![project_line(line, fact)?]))
            }
            Coordinates::MultiLineString(lines) => Self::Lines(MultiLineString(
                lines
                    .into_iter()
                    .map(|line| project_line(line, fact))
                    .collect::<Result<_, _>>()?,
            )),
            Coordinates::Polygon(rings) => {
                Self::Polygons(MultiPolygon(vec![project_polygon(rings, fact)?]))
            }
            Coordinates::MultiPolygon(polygons) => Self::Polygons(MultiPolygon(
                polygons
                    .into_iter()
                    .map(|rings| project_polygon(rings, fact))
                    .collect::<Result<_, _>>()?,
            )),
        })
    }

    pub(super) fn tiles(&self, zoom: u8) -> Result<Vec<(Address, Shape)>, TileError> {
        if let Self::Point(point) = self {
            return Ok(super::projection::placements(*point, zoom)
                .into_iter()
                .map(|(address, p)| (address, Self::Point((f64::from(p.x), f64::from(p.y)))))
                .collect());
        }
        let bounds = match self {
            Self::Lines(lines) => lines.bounding_rect(),
            Self::Polygons(polygons) => polygons.bounding_rect(),
            Self::Point(_) => None,
        };
        let Some(bounds) = bounds else {
            return Ok(Vec::new());
        };
        placement::tiles(self, bounds, zoom)
    }

    fn clip(&self, bounds: Rect) -> Self {
        let mask = bounds.to_polygon();
        match self {
            Self::Lines(lines) => Self::Lines(mask.clip(lines, false)),
            Self::Polygons(polygons) => Self::Polygons(polygons.intersection(&mask)),
            Self::Point(point) => Self::Point(*point),
        }
    }
}

fn project_line(points: Vec<[f64; 2]>, fact: FactId) -> Result<LineString, TileError> {
    points
        .into_iter()
        .map(|[longitude, latitude]| {
            let (_, y) = super::projection::project(
                LonLat {
                    longitude,
                    latitude,
                },
                fact,
            )?;
            // Polygon and line endpoints at +180 must remain on the eastern edge.
            Ok(Coord {
                x: (longitude + 180.0) / 360.0,
                y,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(LineString)
}

fn project_polygon(rings: Vec<Vec<[f64; 2]>>, fact: FactId) -> Result<Polygon, TileError> {
    let mut rings = rings.into_iter().map(|line| project_line(line, fact));
    let exterior = rings
        .next()
        .transpose()?
        .ok_or(TileError::Geometry("polygon has no exterior"))?;
    Ok(Polygon::new(exterior, rings.collect::<Result<_, _>>()?))
}
