//! Namespace-separated identities derived from enduring semantic keys.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

// Changing either namespace would invalidate published Chronicle references.
const ENTITY_NAMESPACE: Uuid = Uuid::from_u128(0x91b6526c_48db_5d1f_bf72_96c30bcad102);
const FACT_NAMESPACE: Uuid = Uuid::from_u128(0x6510383a_b531_5171_9faa_7b225be99252);

/// Invalid semantic key, retained so import errors identify the offending row.
#[derive(Debug, Error)]
#[error("invalid semantic key {0:?}: use slash-separated lowercase ASCII segments")]
pub struct InvalidKey(pub String);

fn validate_key(key: &str) -> Result<(), InvalidKey> {
    if key.is_empty()
        || key.split('/').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
    {
        return Err(InvalidKey(key.to_owned()));
    }
    Ok(())
}

/// Permanent geographical entity identity, independent of display names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub Uuid);

impl EntityId {
    /// Derive a UUIDv5 from a canonical semantic key.
    pub fn from_key(key: &str) -> Result<Self, InvalidKey> {
        validate_key(key)?;
        Ok(Self(Uuid::new_v5(&ENTITY_NAMESPACE, key.as_bytes())))
    }

    /// Two hexadecimal digits used for static index sharding.
    pub fn shard(self) -> String {
        self.0.simple().to_string().chars().take(2).collect()
    }
}

/// Permanent evidence identity; multiple records may describe one entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FactId(pub Uuid);

impl FactId {
    /// Derive an identity in the evidence namespace.
    pub fn from_key(key: &str) -> Result<Self, InvalidKey> {
        validate_key(key)?;
        Ok(Self(Uuid::new_v5(&FACT_NAMESPACE, key.as_bytes())))
    }
}

#[cfg(test)]
mod tests;
