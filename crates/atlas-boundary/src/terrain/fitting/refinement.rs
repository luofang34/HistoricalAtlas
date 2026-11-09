use super::{AreaTarget, FitState, TerrainError, partition, review};
use geo::{BooleanOps, MapCoords, MultiPolygon};

pub(super) fn refine(
    state: &mut FitState<'_>,
    targets: &[&AreaTarget],
) -> Result<(), TerrainError> {
    for _ in 0..state.labels.len() {
        let mut areas = vec![None; state.request.units];
        for target in targets {
            *areas
                .get_mut(target.unit)
                .ok_or(TerrainError::Cell(target.unit))? =
                Some((state.area(target.unit)?, *target));
        }
        if areas.iter().flatten().all(|(a, target)| target.accepts(*a)) {
            return Ok(());
        }
        let candidates = candidates(state, &areas)?;
        let mut moved = false;
        for (_, cell, from, to) in candidates {
            if !crate::terrain::connectivity::removable(state.request, &state.labels, cell, from) {
                continue;
            }
            let before = residual_pair(&areas, from, to, 0.);
            *state.labels.get_mut(cell).ok_or(TerrainError::Cell(cell))? = Some(to);
            let mut after = 0.;
            for unit in [from, to] {
                if let Some((_, target)) = areas.get(unit).copied().flatten() {
                    let actual = state.area(unit)?;
                    after += residual(actual, target);
                }
            }
            if after + 1e-9 < before
                && crate::terrain::connectivity::shared_neighbors(state.request, &state.labels)?
            {
                moved = true;
                break;
            }
            *state.labels.get_mut(cell).ok_or(TerrainError::Cell(cell))? = Some(from);
        }
        if !moved {
            return Ok(());
        }
    }
    Ok(())
}

type Candidate = (f64, usize, usize, usize);

fn candidates(
    state: &FitState<'_>,
    areas: &[Option<(f64, &AreaTarget)>],
) -> Result<Vec<Candidate>, TerrainError> {
    let grid = &state.request.grid;
    let mut result = Vec::new();
    for (cell, owner) in state.labels.iter().enumerate() {
        let Some(from) = *owner else { continue };
        if state.fixed.get(cell).is_some_and(Option::is_some) {
            continue;
        }
        for next in partition::neighbors(cell, grid.width, grid.height) {
            let Some(to) = state.labels.get(next).copied().flatten() else {
                continue;
            };
            if from == to || residual_pair(areas, from, to, 0.) == 0. {
                continue;
            }
            let row = cell / grid.width;
            let col = cell % grid.width;
            let clipped = MultiPolygon(vec![grid.rectangle(row, col, col + 1).to_polygon()])
                .intersection(state.envelope)
                .map_coords(|point| {
                    let angular = crate::terrain::unproject(point);
                    geo::Coord {
                        x: angular.x,
                        y: angular.y,
                    }
                });
            let area = review::area_square_km(&clipped);
            if residual_pair(areas, from, to, area) + 1e-9 >= residual_pair(areas, from, to, 0.) {
                continue;
            }
            result.push((
                partition::edge_cost(state.request, cell, next)?,
                cell,
                from,
                to,
            ));
        }
    }
    result.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.3.cmp(&b.3))
    });
    Ok(result)
}

fn residual(area: f64, target: &AreaTarget) -> f64 {
    ((area - target.square_km).abs() - target.tolerance_square_km).max(0.)
}

fn residual_pair(
    areas: &[Option<(f64, &AreaTarget)>],
    from: usize,
    to: usize,
    transferred: f64,
) -> f64 {
    [(from, -transferred), (to, transferred)]
        .iter()
        .filter_map(|(unit, delta)| {
            areas
                .get(*unit)
                .copied()
                .flatten()
                .map(|(area, target)| residual(area + delta, target))
        })
        .sum()
}
