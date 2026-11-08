use std::collections::BTreeMap;

use atlas_core::{assertion::TemporalValue, identity::EntityId};
use atlas_ledger::{
    model::{Dataset, SourceCatalog},
    timeline::Timeline,
};

use super::{NodalError, NodalScope, RelationEvidence, RelationKind, TerritoryLink};

pub(super) struct Claims {
    pub subjects: BTreeMap<EntityId, Vec<RelationEvidence>>,
    pub links: Vec<TerritoryLink>,
}

pub(super) fn select(
    catalog: &SourceCatalog,
    data: &Dataset,
    scope: &NodalScope,
) -> Result<Claims, NodalError> {
    let mut claims = Claims {
        subjects: BTreeMap::new(),
        links: Vec::new(),
    };
    match scope {
        NodalScope::Source { source_id } => {
            if !catalog.sources.iter().any(|source| source.id == *source_id) {
                return Err(NodalError::UnknownSource(source_id.clone()));
            }
            let mut records: Vec<_> = data
                .records
                .iter()
                .filter(|record| record.checked && record.source_id == *source_id)
                .collect();
            records.sort_by_key(|record| record.id);
            for record in records {
                claims.add(
                    record.entity_id,
                    record.observation.parent_id,
                    record.observation.seat_id,
                    RelationEvidence::Record {
                        record_id: record.id,
                    },
                );
            }
        }
        NodalScope::Time { at, selection } => {
            let timeline = Timeline::new(catalog, data)?;
            for entity in timeline.at(*at, *selection)?.entities {
                for candidate in entity
                    .properties
                    .iter()
                    .flat_map(|property| &property.candidates)
                {
                    if let TemporalValue::Hierarchy(value) = &candidate.assertion.value {
                        claims.add(
                            entity.entity_id,
                            value.parent_id,
                            value.seat_id,
                            RelationEvidence::Estimate {
                                estimate_id: candidate.estimate_id,
                                basis: candidate.basis.to_vec(),
                                validity: candidate.assertion.validity.clone(),
                                membership: candidate.membership,
                            },
                        );
                    }
                }
            }
        }
    }
    claims
        .links
        .sort_by_key(|link| (link.from, link.to, link.relation));
    Ok(claims)
}

impl Claims {
    fn add(
        &mut self,
        subject: EntityId,
        parent: Option<EntityId>,
        seat: Option<EntityId>,
        evidence: RelationEvidence,
    ) {
        self.subjects
            .entry(subject)
            .or_default()
            .push(evidence.clone());
        if let Some(parent) = parent {
            self.links.push(TerritoryLink {
                from: parent,
                to: subject,
                relation: RelationKind::Subordinate,
                evidence: evidence.clone(),
            });
        }
        if let Some(seat) = seat {
            self.links.push(TerritoryLink {
                from: subject,
                to: seat,
                relation: RelationKind::Seat,
                evidence,
            });
        }
    }
}
