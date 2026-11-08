use atlas_core::identity::FactId;
use atlas_geo::coordinates::LonLat;
use serde::Serialize;

use super::TileError;

pub(super) const EXTENT: u32 = 4096;
const BUFFER: f64 = 64.0;
const MAX_LATITUDE: f64 = 85.051_128_779_806_6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct Address {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Position {
    pub x: i32,
    pub y: i32,
}

pub(super) fn project(point: LonLat, record: FactId) -> Result<(f64, f64), TileError> {
    if point.latitude.abs() > MAX_LATITUDE {
        return Err(TileError::PolarCoordinate {
            record,
            latitude: point.latitude,
        });
    }
    let x = ((point.longitude + 180.0) / 360.0).rem_euclid(1.0);
    let y = (1.0 - point.latitude.to_radians().tan().asinh() / std::f64::consts::PI) / 2.0;
    Ok((x, y.clamp(0.0, 1.0)))
}

pub(super) fn placements(projected: (f64, f64), zoom: u8) -> Vec<(Address, Position)> {
    let side = 1_u32 << zoom;
    let world_x = projected.0 * f64::from(side);
    let world_y = projected.1 * f64::from(side);
    let center_x = world_x.floor() as i64;
    let center_y = world_y.floor() as i64;
    let mut result = Vec::new();
    for x in center_x - 1..=center_x + 1 {
        for y in center_y - 1..=center_y + 1 {
            if !(0..i64::from(side)).contains(&y) {
                continue;
            }
            let local_x = (world_x - x as f64) * f64::from(EXTENT);
            let local_y = (world_y - y as f64) * f64::from(EXTENT);
            let domain = -BUFFER..=f64::from(EXTENT) + BUFFER;
            if domain.contains(&local_x) && domain.contains(&local_y) {
                result.push((
                    Address {
                        z: zoom,
                        x: x.rem_euclid(i64::from(side)) as u32,
                        y: y as u32,
                    },
                    Position {
                        x: local_x.round() as i32,
                        y: local_y.round() as i32,
                    },
                ));
            }
        }
    }
    result
}
