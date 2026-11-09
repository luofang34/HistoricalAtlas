use super::{TerrainError, TerrainRequest, connectivity, partition};
use std::collections::{BTreeSet, BinaryHeap};

pub(super) fn establish(
    request: &TerrainRequest,
    labels: &mut [Option<usize>],
    fixed: &mut [Option<usize>],
) -> Result<(), TerrainError> {
    if connectivity::shared_neighbors(request, labels)? {
        return Ok(());
    }
    let mut pins = connectivity::seed_cells(request)?;
    let pairs: BTreeSet<_> = request
        .neighbors
        .iter()
        .map(|[a, b]| [(*a).min(*b), (*a).max(*b)])
        .collect();
    let mut retained = interfaces(request, labels);
    for [a, b] in pairs {
        if retained.contains(&[a, b]) {
            continue;
        }
        let mut options = Vec::new();
        for (from, to) in [(a, b), (b, a)] {
            if let Some(path) = viable_path(request, labels, fixed, &retained, from, to)? {
                options.push(path);
            }
        }
        options.sort_by(|a, b| a.cost.total_cmp(&b.cost).then_with(|| a.unit.cmp(&b.unit)));
        let path = options.into_iter().next().ok_or_else(|| TerrainError::Input(format!(
            "cited neighbor units {a} and {b} cannot be connected without splitting a unit or moving a seed"
        )))?;
        for &cell in &path.cells {
            *labels.get_mut(cell).ok_or(TerrainError::Cell(cell))? = Some(path.unit);
            *pins.get_mut(cell).ok_or(TerrainError::Cell(cell))? = Some(path.unit);
        }
        *pins
            .get_mut(path.endpoint)
            .ok_or(TerrainError::Cell(path.endpoint))? =
            labels.get(path.endpoint).copied().flatten();
        let mut routes = pins.clone();
        connectivity::preserve(request, labels, &mut routes)?;
        for (target, owner) in fixed.iter_mut().zip(routes) {
            if owner.is_some_and(|unit| unit == a || unit == b) {
                *target = owner;
            }
        }
        retained = interfaces(request, labels);
    }
    Ok(())
}

struct Path {
    cost: f64,
    unit: usize,
    cells: Vec<usize>,
    endpoint: usize,
}

fn viable_path(
    request: &TerrainRequest,
    labels: &[Option<usize>],
    fixed: &[Option<usize>],
    retained: &BTreeSet<[usize; 2]>,
    from: usize,
    to: usize,
) -> Result<Option<Path>, TerrainError> {
    let mut blocked = BTreeSet::new();
    while let Some(mut path) = shortest(request, labels, fixed, &blocked, from, to)? {
        let last_corridor_cell = path.cells.last().copied();
        let mut candidate = labels.to_vec();
        for &cell in &path.cells {
            *candidate.get_mut(cell).ok_or(TerrainError::Cell(cell))? = Some(from);
        }
        let rejected = complete_path(request, labels, &mut candidate, fixed, &mut path)?;
        if rejected.is_none() && retained.is_subset(&interfaces(request, &candidate)) {
            return Ok(Some(path));
        }
        let cell = rejected
            .or(last_corridor_cell)
            .ok_or_else(|| TerrainError::Input("empty neighbor connection path".into()))?;
        blocked.insert(cell);
    }
    Ok(None)
}

fn complete_path(
    request: &TerrainRequest,
    labels: &[Option<usize>],
    candidate: &mut [Option<usize>],
    fixed: &[Option<usize>],
    path: &mut Path,
) -> Result<Option<usize>, TerrainError> {
    let affected: BTreeSet<_> = path
        .cells
        .iter()
        .filter_map(|&cell| labels.get(cell).copied().flatten())
        .collect();
    for unit in affected {
        let parts = components(request, candidate, unit);
        let protected = parts
            .iter()
            .filter(|part| {
                part.iter()
                    .any(|&cell| fixed.get(cell) == Some(&Some(unit)))
            })
            .count();
        if protected > components(request, labels, unit).len() {
            return Ok(path
                .cells
                .iter()
                .rev()
                .find(|&&cell| labels.get(cell) == Some(&Some(unit)))
                .copied());
        }
        // A corridor may cut off an unseeded tip without separating located members.
        for part in parts {
            if part
                .iter()
                .any(|&cell| fixed.get(cell) == Some(&Some(unit)))
            {
                continue;
            }
            for cell in part {
                *candidate.get_mut(cell).ok_or(TerrainError::Cell(cell))? = Some(path.unit);
                path.cells.push(cell);
            }
        }
    }
    Ok(None)
}

fn components(request: &TerrainRequest, labels: &[Option<usize>], unit: usize) -> Vec<Vec<usize>> {
    let mut visited = vec![false; labels.len()];
    let mut parts = Vec::new();
    for (cell, owner) in labels.iter().enumerate() {
        if *owner != Some(unit) || visited.get(cell) == Some(&true) {
            continue;
        }
        let mut part = Vec::new();
        let mut pending = vec![cell];
        while let Some(current) = pending.pop() {
            let Some(seen) = visited.get_mut(current) else {
                continue;
            };
            if *seen {
                continue;
            }
            *seen = true;
            part.push(current);
            pending.extend(
                partition::neighbors(current, request.grid.width, request.grid.height)
                    .into_iter()
                    .filter(|&next| labels.get(next) == Some(&Some(unit))),
            );
        }
        parts.push(part);
    }
    parts
}

fn shortest(
    request: &TerrainRequest,
    labels: &[Option<usize>],
    fixed: &[Option<usize>],
    blocked: &BTreeSet<usize>,
    from: usize,
    to: usize,
) -> Result<Option<Path>, TerrainError> {
    let mut costs = vec![f64::INFINITY; labels.len()];
    let mut parents = vec![None; labels.len()];
    let mut queue = BinaryHeap::new();
    for (cell, owner) in labels.iter().enumerate() {
        if *owner == Some(from) {
            *costs.get_mut(cell).ok_or(TerrainError::Cell(cell))? = 0.;
            queue.push(partition::Visit {
                cost: 0.,
                unit: from,
                cell,
            });
        }
    }
    while let Some(visit) = queue.pop() {
        if costs.get(visit.cell) != Some(&visit.cost) {
            continue;
        }
        for next in partition::neighbors(visit.cell, request.grid.width, request.grid.height) {
            if labels.get(next) == Some(&Some(to)) {
                return Ok(Some(Path {
                    cost: visit.cost,
                    unit: from,
                    cells: trace(labels, &parents, visit.cell, from)?,
                    endpoint: next,
                }));
            }
            if labels.get(next).is_none_or(Option::is_none)
                || fixed.get(next).is_some_and(Option::is_some)
                || blocked.contains(&next)
            {
                continue;
            }
            let cost = visit.cost + partition::edge_cost(request, visit.cell, next)?;
            let previous = costs.get_mut(next).ok_or(TerrainError::Cell(next))?;
            if cost < *previous {
                *previous = cost;
                *parents.get_mut(next).ok_or(TerrainError::Cell(next))? = Some(visit.cell);
                queue.push(partition::Visit {
                    cost,
                    unit: from,
                    cell: next,
                });
            }
        }
    }
    Ok(None)
}

fn trace(
    labels: &[Option<usize>],
    parents: &[Option<usize>],
    mut cell: usize,
    unit: usize,
) -> Result<Vec<usize>, TerrainError> {
    let mut cells = vec![];
    while labels.get(cell) != Some(&Some(unit)) {
        cells.push(cell);
        cell = parents
            .get(cell)
            .copied()
            .flatten()
            .ok_or(TerrainError::Cell(cell))?;
    }
    cells.reverse();
    Ok(cells)
}

fn interfaces(request: &TerrainRequest, labels: &[Option<usize>]) -> BTreeSet<[usize; 2]> {
    let wanted: BTreeSet<_> = request
        .neighbors
        .iter()
        .map(|[a, b]| [(*a).min(*b), (*a).max(*b)])
        .collect();
    let mut result = BTreeSet::new();
    for (cell, owner) in labels.iter().enumerate() {
        let Some(a) = *owner else { continue };
        for next in partition::neighbors(cell, request.grid.width, request.grid.height) {
            if let Some(b) = labels.get(next).copied().flatten() {
                let pair = [a.min(b), a.max(b)];
                if wanted.contains(&pair) {
                    result.insert(pair);
                }
            }
        }
    }
    result
}
