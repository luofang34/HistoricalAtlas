use super::{TerrainError, TerrainRequest};
use geo::{Intersects, MultiPolygon};
use std::{cmp::Ordering, collections::BinaryHeap};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Visit {
    pub cost: f64,
    pub unit: usize,
    pub cell: usize,
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

pub(super) fn active(request: &TerrainRequest, envelope: &MultiPolygon<f64>) -> Vec<bool> {
    let grid = &request.grid;
    (0..grid.elevations_m.len())
        .map(|i| {
            let (y, x) = (i / grid.width, i % grid.width);
            envelope.intersects(&grid.rectangle(y, x, x + 1).to_polygon())
        })
        .collect()
}

pub(super) fn assign(
    request: &TerrainRequest,
    active: &[bool],
    offsets: &[f64],
    fixed: &[Option<usize>],
) -> Result<Vec<Option<usize>>, TerrainError> {
    let grid = &request.grid;
    let mut labels = vec![None; active.len()];
    let mut costs = vec![f64::INFINITY; active.len()];
    let mut queue = BinaryHeap::new();
    for (index, seed) in request.seeds.iter().enumerate() {
        let cell = grid.cell(seed.point).ok_or(TerrainError::Seed(index))?;
        *labels.get_mut(cell).ok_or(TerrainError::Cell(cell))? = Some(seed.unit);
        let offset = *offsets.get(seed.unit).ok_or(TerrainError::Seed(index))?;
        *costs.get_mut(cell).ok_or(TerrainError::Cell(cell))? = offset;
        queue.push(Visit {
            cost: offset,
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
            if active.get(next) != Some(&true)
                || fixed
                    .get(next)
                    .is_some_and(|u| u.is_some_and(|u| u != visit.unit))
            {
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

pub(super) fn neighbors(cell: usize, width: usize, height: usize) -> Vec<usize> {
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

pub(super) fn edge_cost(request: &TerrainRequest, a: usize, b: usize) -> Result<f64, TerrainError> {
    let grid = &request.grid;
    let za = f64::from(*grid.elevations_m.get(a).ok_or(TerrainError::Cell(a))?);
    let zb = f64::from(*grid.elevations_m.get(b).ok_or(TerrainError::Cell(b))?);
    let y = grid.north_m - (a / grid.width) as f64 * grid.cell_m;
    let horizontal = grid.cell_m / (y / super::RADIUS).cosh();
    Ok(horizontal + request.slope_weight * (za - zb).abs())
}
