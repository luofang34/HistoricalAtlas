//! Quantitative territory descriptions and named adjoining units.

use atlas_core::identity::EntityId;
use serde::{Deserialize, Serialize};

/// Area and neighbors stated together by one source description.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerritoryObservation {
    /// Printed area converted to square kilometres.
    pub area_square_km: f64,
    /// Named adjoining units, without assuming the list is exhaustive.
    pub neighbors: Vec<NeighborObservation>,
}

/// One explicitly named adjoining administrative unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeighborObservation {
    /// Name as used in this source passage.
    pub name: String,
    /// Corresponding ledger identity, when established.
    pub entity_id: Option<EntityId>,
}
