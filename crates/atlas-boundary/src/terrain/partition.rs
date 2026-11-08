use super::{TerrainError, TerrainRequest};
use geo::{Intersects, MultiPolygon};
use std::{cmp::Ordering, collections::BinaryHeap};

#[derive(Clone, Copy, PartialEq)]
struct Visit {
    cost: f64,
    unit: usize,
    cell: usize,
}

impl Eq for Visit {}
impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.unit.cmp(&self.unit))
            .then_with(|| other.cell.cmp(&self.cell))
    }
}
impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) fn assign(
    request: &TerrainRequest,
    envelope: &MultiPolygon<f64>,
) -> Result<Vec<Option<usize>>, TerrainError> {
    let grid = &request.grid;
    let active: Vec<_> = (0..grid.elevations_m.len())
        .map(|i| {
            let (y, x) = (i / grid.width, i % grid.width);
            envelope.intersects(&grid.rectangle(y, x, x + 1).to_polygon())
        })
        .collect();
    let mut labels = vec![None; active.len()];
    let mut costs = vec![f64::INFINITY; active.len()];
    let mut queue = BinaryHeap::new();
    for (index, seed) in request.seeds.iter().enumerate() {
        let cell = grid.cell(seed.point).ok_or(TerrainError::Seed(index))?;
        let owner = labels.get_mut(cell).ok_or(TerrainError::Cell(cell))?;
        if let Some(first) = *owner
            && first != seed.unit
        {
            return Err(TerrainError::Conflict {
                cell,
                first,
                second: seed.unit,
            });
        }
        *owner = Some(seed.unit);
        *costs.get_mut(cell).ok_or(TerrainError::Cell(cell))? = 0.0;
        queue.push(Visit {
            cost: 0.0,
            unit: seed.unit,
            cell,
        });
    }
    while let Some(visit) = queue.pop() {
        if costs.get(visit.cell) != Some(&visit.cost)
            || labels.get(visit.cell) != Some(&Some(visit.unit))
        {
            continue;
        }
        for next in neighbors(visit.cell, grid.width, grid.height) {
            if active.get(next) != Some(&true) {
                continue;
            }
            let cost = visit.cost + edge_cost(request, visit.cell, next)?;
            let distance = costs.get_mut(next).ok_or(TerrainError::Cell(next))?;
            let owner = labels.get_mut(next).ok_or(TerrainError::Cell(next))?;
            if cost < *distance || (cost == *distance && owner.is_some_and(|u| visit.unit < u)) {
                *distance = cost;
                *owner = Some(visit.unit);
                queue.push(Visit {
                    cost,
                    unit: visit.unit,
                    cell: next,
                });
            }
        }
    }
    let unreached = active
        .iter()
        .zip(&labels)
        .filter(|(a, l)| **a && l.is_none())
        .count();
    if unreached > 0 {
        return Err(TerrainError::Unreached(unreached));
    }
    Ok(labels)
}

fn neighbors(cell: usize, width: usize, height: usize) -> Vec<usize> {
    let (row, col) = (cell / width, cell % width);
    let mut result = Vec::with_capacity(4);
    if row > 0 {
        result.push(cell - width);
    }
    if row + 1 < height {
        result.push(cell + width);
    }
    if col > 0 {
        result.push(cell - 1);
    }
    if col + 1 < width {
        result.push(cell + 1);
    }
    result
}

fn edge_cost(request: &TerrainRequest, a: usize, b: usize) -> Result<f64, TerrainError> {
    let grid = &request.grid;
    let za = f64::from(*grid.elevations_m.get(a).ok_or(TerrainError::Cell(a))?);
    let zb = f64::from(*grid.elevations_m.get(b).ok_or(TerrainError::Cell(b))?);
    let y = grid.north_m - (a / grid.width) as f64 * grid.cell_m;
    let horizontal = grid.cell_m / (y / super::RADIUS).cosh();
    Ok(horizontal + request.slope_weight * (za - zb).abs())
}
