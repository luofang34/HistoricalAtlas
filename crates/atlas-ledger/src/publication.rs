//! Redirect integrity for previously published permanent entity links.

use std::collections::{BTreeMap, BTreeSet};

use atlas_core::identity::EntityId;
use serde::{Deserialize, Serialize};

use crate::validation::error::fail;

use crate::{model::Dataset, validation::ValidationError};

/// Stable redirects for entity merges and identity migrations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Aliases(pub BTreeMap<EntityId, EntityId>);

/// Entity IDs promised by earlier static publications.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Published(pub BTreeSet<EntityId>);

impl Aliases {
    /// Resolve a link and detect cycles rather than silently losing history.
    pub fn resolve(&self, id: EntityId) -> Result<EntityId, ValidationError> {
        let mut cursor = id;
        let mut visited = BTreeSet::new();
        while let Some(next) = self.0.get(&cursor) {
            if !visited.insert(cursor) {
                return Err(fail(format!("{:?}", id), "alias cycle"));
            }
            cursor = *next;
        }
        Ok(cursor)
    }
}

/// Reject missing links, dangling redirects, and redirects shadowing live entities.
pub fn verify_publication(
    data: &Dataset,
    aliases: &Aliases,
    published: &Published,
) -> Result<(), ValidationError> {
    let current: BTreeSet<_> = data.entities.iter().map(|e| e.id).collect();
    for id in aliases.0.keys() {
        if current.contains(id) {
            return Err(fail(format!("{:?}", id), "alias shadows a live entity"));
        }
    }
    for id in published.0.iter().chain(aliases.0.keys()) {
        if !current.contains(&aliases.resolve(*id)?) {
            return Err(fail(
                format!("{:?}", id),
                "published link does not resolve to a live entity",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
