use super::{TerrainError, TerrainRequest, connectivity, partition, review, vectorize};
use geo::MultiPolygon;
use std::collections::BTreeSet;

/// Area constraint for a unit in the current angular frame.
#[derive(Debug, Clone)]
pub struct AreaTarget {
    /// Index into the ordered unit list.
    pub unit: usize,
    /// Area reported by the source, in square kilometres.
    pub square_km: f64,
    /// Accepted absolute difference between modeled and source areas.
    pub tolerance_square_km: f64,
}

pub(super) fn assign(
    request: &TerrainRequest,
    envelope: &MultiPolygon<f64>,
) -> Result<Vec<Option<usize>>, TerrainError> {
    validate(request)?;
    let active = partition::active(request, envelope);
    let offsets = vec![0.; request.units];
    let mut fixed = connectivity::seed_cells(request)?;
    let labels = partition::assign(request, &active, &offsets, &fixed)?;
    if request.areas.is_empty() {
        return Ok(labels);
    }
    connectivity::preserve(request, &labels, &mut fixed)?;
    let mut state = FitState {
        request,
        envelope,
        active,
        offsets,
        labels,
        fixed,
    };
    let mut targets: Vec<_> = request.areas.iter().collect();
    targets.sort_by_key(|target| target.unit);
    for _ in 0..8 {
        for target in &targets {
            state.fit(target)?;
        }
        let mut failure = None;
        for target in &targets {
            let area = state.area(target.unit)?;
            if !target.accepts(area) {
                failure = Some(target.failure(area));
            }
        }
        if failure.is_none() {
            return Ok(state.labels);
        }
    }
    for target in targets {
        let area = state.area(target.unit)?;
        if !target.accepts(area) {
            return Err(target.failure(area));
        }
    }
    Ok(state.labels)
}

fn validate(request: &TerrainRequest) -> Result<(), TerrainError> {
    let mut units = BTreeSet::new();
    for target in &request.areas {
        if target.unit >= request.units
            || !units.insert(target.unit)
            || !target.square_km.is_finite()
            || target.square_km <= 0.
            || !target.tolerance_square_km.is_finite()
            || target.tolerance_square_km <= 0.
            || target.tolerance_square_km >= target.square_km
        {
            return Err(TerrainError::Input(
                "area targets need distinct units, positive areas, and smaller positive tolerances"
                    .into(),
            ));
        }
    }
    Ok(())
}

struct FitState<'a> {
    request: &'a TerrainRequest,
    envelope: &'a MultiPolygon<f64>,
    active: Vec<bool>,
    offsets: Vec<f64>,
    labels: Vec<Option<usize>>,
    fixed: Vec<Option<usize>>,
}

impl FitState<'_> {
    fn area(&self, unit: usize) -> Result<f64, TerrainError> {
        let region = vectorize::region(self.request, &self.labels, self.envelope, unit)?;
        let polygon = crate::polygon::resolve(&region.vertices, &region.polygons)?;
        Ok(review::area_square_km(&polygon))
    }

    fn fit(&mut self, target: &AreaTarget) -> Result<(), TerrainError> {
        let (mut lower, mut upper) = (None, None);
        let mut step = self.request.grid.cell_m;
        for _ in 0..40 {
            let area = self.area(target.unit)?;
            if target.accepts(area) {
                return Ok(());
            }
            let offset = self
                .offsets
                .get_mut(target.unit)
                .ok_or(TerrainError::Cell(target.unit))?;
            if area > target.square_km {
                lower = Some(*offset);
            } else {
                upper = Some(*offset);
            }
            // Additive seed costs retain the same terrain cost on every shared edge.
            *offset = match (lower, upper) {
                (Some(a), Some(b)) => a + (b - a) / 2.,
                (Some(a), None) => a + step,
                (None, Some(b)) => b - step,
                (None, None) => *offset,
            };
            step *= 2.;
            self.labels =
                partition::assign(self.request, &self.active, &self.offsets, &self.fixed)?;
        }
        // Neighboring offsets can make this target attainable in a later sweep.
        Ok(())
    }
}

impl AreaTarget {
    fn accepts(&self, area: f64) -> bool {
        (area - self.square_km).abs() <= self.tolerance_square_km
    }

    fn failure(&self, area: f64) -> TerrainError {
        TerrainError::AreaFit {
            unit: self.unit,
            actual: area,
            target: self.square_km,
            tolerance: self.tolerance_square_km,
        }
    }
}

#[cfg(test)]
mod tests;
