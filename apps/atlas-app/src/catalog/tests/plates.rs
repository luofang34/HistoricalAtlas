//! Plate models from compiled publication artifacts.

use atlas_ledger::plates::PlateModelDocument;
use std::{collections::BTreeMap, path::PathBuf};

/// Every model the compiler listed in `plates/models.json`.
#[cfg(test)]
pub(super) fn published(
    output: &BTreeMap<PathBuf, Vec<u8>>,
) -> BTreeMap<String, PlateModelDocument> {
    let models: Vec<String> =
        serde_json::from_slice(&output[&PathBuf::from("plates/models.json")]).unwrap();
    models
        .into_iter()
        .map(|id| {
            let path = PathBuf::from(format!("plates/{id}.json"));
            (id, serde_json::from_slice(&output[&path]).unwrap())
        })
        .collect()
}
