//! External Chronicle events anchored to published geographical identities.

use std::collections::BTreeSet;

use atlas_core::{
    identity::EntityId,
    time::{EpochTime, TemporalSpan, TimeError, TimeMembership},
};
use atlas_geo::body::BodyId;
use atlas_ledger::timeline::TimeSelection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::catalog::Atlas;

mod features;

/// Event package supplied by an external chronicle application.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayPackage {
    /// Package schema version; the supported version is 1.
    pub version: u32,
    /// Human-readable name of the external collection.
    pub title: String,
    /// Events with exact atlas references and bounded or unknown dates.
    pub events: Vec<ChronicleEvent>,
}

/// One external event, kept separate from the atlas evidence ledger.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChronicleEvent {
    /// Stable identifier within its package.
    pub id: String,
    /// Plain-text event title.
    pub title: String,
    /// Permanent entity identities, including supported published redirects.
    pub entities: Vec<EntityId>,
    /// TT interval with an exclusive end, using the atlas uncertainty model.
    pub validity: TemporalSpan,
    /// Plain-text account supplied by the package author.
    pub description: Option<String>,
    /// HTTP(S) link to the external event's source or full account.
    pub url: Option<String>,
}

/// Completed filesystem or browser request ready for application validation.
pub struct OverlayInput {
    /// File path or resolved request URL shown with the package.
    pub origin: String,
    /// JSON text, or a transport error for display in the event panel.
    pub content: Result<String, String>,
}

/// Validated event package and its loading location.
pub struct Overlay {
    origin: String,
    package: OverlayPackage,
}

impl Overlay {
    /// Decode a package and resolve every geographical reference before use.
    pub fn parse(atlas: &Atlas, origin: String, json: &str) -> Result<Self, OverlayError> {
        if json.len() >= 4_000_000 {
            return Err(OverlayError::Oversize(json.len()));
        }
        let package: OverlayPackage = serde_json::from_str(json)?;
        if package.version != 1 {
            return Err(OverlayError::Version(package.version));
        }
        if package.title.trim().is_empty() {
            return Err(OverlayError::EmptyTitle);
        }
        let mut ids = BTreeSet::new();
        for event in &package.events {
            if event.id.trim().is_empty() || !ids.insert(&event.id) {
                return Err(OverlayError::EventId(event.id.clone()));
            }
            validate_event(atlas, event)?;
        }
        Ok(Self { origin, package })
    }

    /// Package metadata and original, unmodified event references.
    pub fn package(&self) -> &OverlayPackage {
        &self.package
    }

    /// File path or URL used to load this package.
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

impl ChronicleEvent {
    /// Evaluate the same endpoint rules as the atlas time-slice compiler.
    pub fn matches(&self, at: EpochTime, selection: TimeSelection) -> Result<bool, TimeError> {
        Ok(match selection {
            TimeSelection::Nominal => self.validity.nominally_contains(at)?,
            TimeSelection::Possible => matches!(
                self.validity.membership(at)?,
                TimeMembership::Possible | TimeMembership::Certain
            ),
            TimeSelection::Certain => self.validity.membership(at)? == TimeMembership::Certain,
        })
    }

    /// Current identities on a selected body, preserving the package's ordering.
    pub fn anchors(&self, atlas: &Atlas, body: BodyId) -> Vec<EntityId> {
        self.entities
            .iter()
            .filter_map(|id| atlas.resolve(*id))
            .filter(|id| {
                atlas
                    .entry(*id)
                    .is_some_and(|entry| entry.entity.body == body)
            })
            .collect()
    }

    /// Representative time for navigation; fully undated events have none.
    pub fn focus_time(&self) -> Result<Option<EpochTime>, TimeError> {
        if let Some(start) = self.validity.start {
            Ok(Some(start.nominal))
        } else {
            self.validity
                .end
                .map(|end| end.nominal.checked_add(-1))
                .transpose()
        }
    }
}

fn validate_event(atlas: &Atlas, event: &ChronicleEvent) -> Result<(), OverlayError> {
    if event.title.trim().is_empty() || event.entities.is_empty() {
        return Err(OverlayError::EventContent(event.id.clone()));
    }
    event
        .validity
        .validate()
        .map_err(|source| OverlayError::Date {
            event: event.id.clone(),
            source,
        })?;
    if let Some(url) = &event.url {
        validate_link(url).map_err(|source| OverlayError::Link {
            event: event.id.clone(),
            source,
        })?;
    }
    let mut targets = BTreeSet::new();
    for id in &event.entities {
        let target = atlas
            .resolve(*id)
            .ok_or_else(|| OverlayError::UnknownEntity {
                event: event.id.clone(),
                entity: *id,
            })?;
        if !targets.insert(target) {
            return Err(OverlayError::RepeatedAnchor {
                event: event.id.clone(),
                entity: target,
            });
        }
    }
    Ok(())
}

/// Check links before exposing them as navigable external resources.
pub fn validate_link(value: &str) -> Result<(), LinkError> {
    let url = url::Url::parse(value)?;
    if matches!(url.scheme(), "http" | "https") && url.has_host() {
        Ok(())
    } else {
        Err(LinkError::Scheme)
    }
}

/// Invalid external link.
#[derive(Debug, Error)]
pub enum LinkError {
    /// URL syntax could not be parsed.
    #[error("invalid URL: {0}")]
    Parse(#[from] url::ParseError),
    /// External event links use HTTP or HTTPS.
    #[error("event link must be an absolute HTTP(S) URL")]
    Scheme,
}

/// External event schema, identity, or temporal validation failure.
#[derive(Debug, Error)]
pub enum OverlayError {
    /// Package exceeds the static artifact size contract.
    #[error("event package is {0} bytes; it must be smaller than 4 MB")]
    Oversize(usize),
    /// JSON could not be decoded.
    #[error("event package JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Package schema is unsupported.
    #[error("unsupported event package version {0}; expected 1")]
    Version(u32),
    /// Collection title is empty.
    #[error("event package requires a title")]
    EmptyTitle,
    /// Event identifiers are empty or repeated.
    #[error("empty or repeated event identifier {0:?}")]
    EventId(String),
    /// An event is missing its title or geographical anchor.
    #[error("event {0:?} requires a title and at least one entity")]
    EventContent(String),
    /// Event dates are inconsistent.
    #[error("event {event:?} has invalid dates: {source}")]
    Date {
        /// Event identifier.
        event: String,
        /// Underlying interval failure.
        #[source]
        source: TimeError,
    },
    /// Event link is invalid.
    #[error("event {event:?} has an invalid link: {source}")]
    Link {
        /// Event identifier.
        event: String,
        /// Underlying URL failure.
        #[source]
        source: LinkError,
    },
    /// An external reference is absent from the loaded atlas.
    #[error("event {event:?} refers to unknown entity {entity:?}")]
    UnknownEntity {
        /// Event identifier.
        event: String,
        /// Unresolved original identity.
        entity: EntityId,
    },
    /// Two references resolve to one current entity.
    #[error("event {event:?} repeats entity {entity:?}")]
    RepeatedAnchor {
        /// Event identifier.
        event: String,
        /// Resolved identity.
        entity: EntityId,
    },
}

#[cfg(test)]
pub(crate) mod tests;
