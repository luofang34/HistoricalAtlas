use super::*;
use atlas_boundary::publication::{GeometryBundle, GeometryReference};

impl Entry {
    /// Attach a decoded geometry bundle; Atlas validates it against the loaded evidence.
    pub(crate) fn attach_geometry(&mut self, bundle: GeometryBundle) {
        self.drawings = bundle.drawings;
        self.boundaries = bundle.boundaries;
        self.derived_boundaries = bundle.derived_boundaries;
        self.lines = bundle.lines;
        self.geometry = None;
    }

    pub(super) fn validate_geometry(
        &self,
        sources: &SourceCatalog,
        data: &Dataset,
    ) -> Result<(), AppError> {
        if let Some(reference) = &self.geometry {
            reference.validate()?;
        }
        validate(
            self.entity.id,
            &self.records,
            &self.lines,
            &self.boundaries,
            &self.derived_boundaries,
            &self.drawings,
            (sources, data),
        )
    }
}

impl Atlas {
    /// Unloaded geometry references on the requested body.
    pub fn geometry_references(
        &self,
        body: BodyId,
    ) -> impl Iterator<Item = (EntityId, &GeometryReference)> {
        self.entries
            .values()
            .filter(move |e| e.entity.body == body)
            .filter_map(|entry| {
                entry
                    .geometry
                    .as_ref()
                    .map(|reference| (entry.entity.id, reference))
            })
    }

    /// Validate and attach asynchronously loaded geometry without rebuilding the evidence index.
    pub fn attach_geometry(
        &mut self,
        id: EntityId,
        bundle: GeometryBundle,
    ) -> Result<(), AppError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(AppError::EvidenceIdentity(id))?;
        validate(
            id,
            &entry.records,
            &bundle.lines,
            &bundle.boundaries,
            &bundle.derived_boundaries,
            &bundle.drawings,
            (&self.sources, &self.data),
        )?;
        entry.attach_geometry(bundle);
        Ok(())
    }
}

fn validate(
    id: EntityId,
    records: &[Record],
    lines: &[atlas_boundary::linear::GeographicLine],
    boundaries: &[atlas_boundary::geographic::GeographicBoundary],
    derived: &[atlas_boundary::terrain::DerivedBoundary],
    drawings: &[atlas_boundary::sheet::SheetTrace],
    (sources, data): (&SourceCatalog, &Dataset),
) -> Result<(), AppError> {
    for line in lines {
        if !records.iter().any(|r| r.id == line.line.record_id) {
            return Err(AppError::EvidenceIdentity(id));
        }
        line.feature(data, None)?;
    }
    for boundary in boundaries {
        if !records.iter().any(|r| r.id == boundary.unit.record_id) {
            return Err(AppError::EvidenceIdentity(id));
        }
        boundary.feature(data, None)?;
    }
    for boundary in derived {
        if !records.iter().any(|r| r.id == boundary.record_id) {
            return Err(AppError::EvidenceIdentity(id));
        }
        boundary.feature(data, None)?;
    }
    for drawing in drawings {
        atlas_boundary::sheet::validate(sources, data, drawing)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
