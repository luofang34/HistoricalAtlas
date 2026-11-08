use super::{TerrainError, TerrainRequest, partition};
use std::collections::BinaryHeap;

pub(super) fn seed_cells(request: &TerrainRequest) -> Result<Vec<Option<usize>>, TerrainError> {
    let mut fixed = vec![None; request.grid.elevations_m.len()];
    for (index, seed) in request.seeds.iter().enumerate() {
        let cell = request
            .grid
            .cell(seed.point)
            .ok_or(TerrainError::Seed(index))?;
        let owner = fixed.get_mut(cell).ok_or(TerrainError::Cell(cell))?;
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
    }
    Ok(fixed)
}

pub(super) fn preserve(
    request: &TerrainRequest,
    labels: &[Option<usize>],
    fixed: &mut [Option<usize>],
) -> Result<(), TerrainError> {
    let seeds: Vec<_> = fixed
        .iter()
        .enumerate()
        .filter_map(|(cell, owner)| owner.map(|unit| (cell, unit)))
        .collect();
    let mut costs = vec![f64::INFINITY; labels.len()];
    let mut parents = vec![None; labels.len()];
    for &(cell, unit) in &seeds {
        if costs.get(cell).is_some_and(|cost| cost.is_finite()) {
            continue;
        }
        paths(request, labels, cell, unit, &mut costs, &mut parents)?;
    }
    // Keep connected seed groups joined as area fitting moves the surrounding edges.
    for (mut cell, unit) in seeds {
        while let Some(parent) = parents.get(cell).copied().flatten() {
            *fixed.get_mut(parent).ok_or(TerrainError::Cell(parent))? = Some(unit);
            cell = parent;
        }
    }
    Ok(())
}

fn paths(
    request: &TerrainRequest,
    labels: &[Option<usize>],
    root: usize,
    unit: usize,
    costs: &mut [f64],
    parents: &mut [Option<usize>],
) -> Result<(), TerrainError> {
    *costs.get_mut(root).ok_or(TerrainError::Cell(root))? = 0.;
    let mut queue = BinaryHeap::from([partition::Visit {
        cost: 0.,
        unit,
        cell: root,
    }]);
    while let Some(visit) = queue.pop() {
        if costs.get(visit.cell) != Some(&visit.cost) {
            continue;
        }
        for next in partition::neighbors(visit.cell, request.grid.width, request.grid.height) {
            if labels.get(next) != Some(&Some(unit)) {
                continue;
            }
            let cost = visit.cost + partition::edge_cost(request, visit.cell, next)?;
            let distance = costs.get_mut(next).ok_or(TerrainError::Cell(next))?;
            if cost < *distance {
                *distance = cost;
                *parents.get_mut(next).ok_or(TerrainError::Cell(next))? = Some(visit.cell);
                queue.push(partition::Visit {
                    cost,
                    unit,
                    cell: next,
                });
            }
        }
    }
    Ok(())
}
