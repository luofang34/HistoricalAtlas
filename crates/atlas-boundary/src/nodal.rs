//! Administrative and seat links with independently retained coordinate observations.

use std::collections::{BTreeMap, BTreeSet};

use atlas_core::{
    identity::{EntityId, FactId},
    time::{EpochTime, TemporalSpan, TimeMembership},
};
use atlas_geo::{body::BodyId, coordinates::CoordinateObservation};
use atlas_ledger::{
    model::{Dataset, SourceCatalog},
    timeline::TimeSelection,
};
use serde::{Deserialize, Serialize};

mod claims;
mod error;
mod geojson;
pub use error::NodalError;

/// Evidence selection for administrative relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NodalScope {
    /// Relationships transcribed from one edition, without an inferred date.
    Source {
        /// Catalogued source edition.
        source_id: String,
    },
    /// Dated hierarchy estimates selected by the shared timeline.
    Time {
        /// TT millisecond epoch.
        at: EpochTime,
        /// Treatment of uncertain interval endpoints.
        selection: TimeSelection,
    },
}

/// Root territory and the evidence used to expand its subordinate network.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodalQuery {
    /// Administrative unit to explore.
    pub entity_id: EntityId,
    /// Relationship selection, independent of coordinate observation dates.
    pub scope: NodalScope,
}

/// An unchanged coordinate observation used to draw a node.
#[derive(Debug, Clone, Serialize)]
pub struct NodeAnchor {
    /// Checked record providing this coordinate.
    pub record_id: FactId,
    /// Original coordinate, notation, datum, and angular precision.
    pub coordinate: CoordinateObservation,
}

/// One enduring identity, including identities without located anchors.
#[derive(Debug, Serialize)]
pub struct TerritoryNode {
    /// Permanent geographical identity.
    pub entity_id: EntityId,
    /// Selected assertions about this subject, including hierarchy without parent or seat.
    pub evidence: Vec<RelationEvidence>,
    /// Checked numerical coordinates in stable record-identity order.
    pub anchors: Vec<NodeAnchor>,
    /// Checked records whose coordinate notation remains unresolved.
    pub unresolved_coordinate_records: Vec<FactId>,
}

/// Meaning of a directed territorial relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// The child is subordinate to the parent.
    Subordinate,
    /// The administrative unit has its seat at the linked place.
    Seat,
}

/// Evidence establishing a relationship, distinct from its drawing anchors.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelationEvidence {
    /// Undated source assertion.
    Record {
        /// Checked record with the parent or seat field.
        record_id: FactId,
    },
    /// A temporal assertion admitted by the query's uncertainty mode.
    Estimate {
        /// Selected interpretation.
        estimate_id: FactId,
        /// Checked records supporting this interpretation.
        basis: Vec<FactId>,
        /// Full interval uncertainty from the interpretation.
        validity: TemporalSpan,
        /// Support at the selected instant.
        membership: TimeMembership,
    },
}

/// One cited relationship; competing assertions remain separate links.
#[derive(Debug, Clone, Serialize)]
pub struct TerritoryLink {
    /// Parent unit or seat-bearing unit.
    pub from: EntityId,
    /// Subordinate entity or seat settlement.
    pub to: EntityId,
    /// Administrative meaning of the link.
    pub relation: RelationKind,
    /// Source or dated interpretation that establishes the relationship.
    pub evidence: RelationEvidence,
}

/// A territory represented by located nodes and cited administrative connections.
#[derive(Debug, Serialize)]
pub struct NodalTerritory {
    /// Request that generated the network.
    pub query: NodalQuery,
    /// Body containing every network member.
    pub body: BodyId,
    /// Identities in stable UUID order, including unlocated members.
    pub nodes: Vec<TerritoryNode>,
    /// Selected administrative and seat relationships.
    pub links: Vec<TerritoryLink>,
}

/// Validated ledger reusable for several territory queries.
pub struct NodalBuilder<'a> {
    catalog: &'a SourceCatalog,
    data: &'a Dataset,
}

impl<'a> NodalBuilder<'a> {
    /// Validate evidence before resolving territorial connections.
    pub fn new(catalog: &'a SourceCatalog, data: &'a Dataset) -> Result<Self, NodalError> {
        atlas_ledger::validation::validate(catalog, data)?;
        Ok(Self { catalog, data })
    }

    /// Expand subordinate units and attach their seats, retaining unlocated members.
    pub fn build(&self, query: NodalQuery) -> Result<NodalTerritory, NodalError> {
        let entity = self
            .data
            .entities
            .iter()
            .find(|entity| entity.id == query.entity_id)
            .ok_or(NodalError::UnknownEntity(query.entity_id))?;
        let claims = claims::select(self.catalog, self.data, &query.scope)?;
        let mut members = BTreeSet::new();
        if claims.subjects.contains_key(&entity.id)
            || claims.links.iter().any(|link| link.from == entity.id)
        {
            members.insert(entity.id);
        }
        let mut pending: Vec<_> = members.iter().copied().collect();
        let mut outgoing: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for link in &claims.links {
            outgoing.entry(link.from).or_default().push(link);
        }
        while let Some(parent) = pending.pop() {
            for link in outgoing.get(&parent).into_iter().flatten() {
                if link.relation == RelationKind::Subordinate && members.insert(link.to) {
                    pending.push(link.to);
                }
            }
        }
        let links: Vec<_> = claims
            .links
            .into_iter()
            .filter(|link| members.contains(&link.from))
            .collect();
        members.extend(links.iter().map(|link| link.to));
        let nodes = members
            .into_iter()
            .map(|id| {
                self.node(
                    id,
                    &query.scope,
                    claims.subjects.get(&id).cloned().unwrap_or_default(),
                )
            })
            .collect();
        Ok(NodalTerritory {
            query,
            body: entity.body,
            nodes,
            links,
        })
    }

    fn node(
        &self,
        entity_id: EntityId,
        scope: &NodalScope,
        evidence: Vec<RelationEvidence>,
    ) -> TerritoryNode {
        let records = self.data.records.iter().filter(|record| {
            record.checked
                && record.entity_id == entity_id
                && match scope {
                    NodalScope::Source { source_id } => record.source_id == *source_id,
                    NodalScope::Time { .. } => true,
                }
        });
        let mut anchors = Vec::new();
        let mut unresolved_coordinate_records = Vec::new();
        for record in records {
            if let Some(coordinate) = &record.observation.coordinate {
                anchors.push(NodeAnchor {
                    record_id: record.id,
                    coordinate: coordinate.clone(),
                });
            }
            if record.observation.unresolved_coordinate.is_some() {
                unresolved_coordinate_records.push(record.id);
            }
        }
        anchors.sort_by_key(|anchor| anchor.record_id);
        unresolved_coordinate_records.sort();
        TerritoryNode {
            entity_id,
            evidence,
            anchors,
            unresolved_coordinate_records,
        }
    }
}

#[cfg(test)]
mod tests;
