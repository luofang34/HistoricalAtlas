//! Plate models from compiled publication artifacts.

use atlas_boundary::publication::decode_concatenated;
use atlas_ledger::plates::PlateModelDocument;
use std::{collections::BTreeMap, path::PathBuf};

/// Every model the compiler listed in `manifest.json`.
#[cfg(test)]
pub(super) fn published(
    output: &BTreeMap<PathBuf, Vec<u8>>,
) -> BTreeMap<String, PlateModelDocument> {
    let manifest = atlas_compile::export::read_manifest(output).unwrap();
    manifest
        .plate_models
        .into_iter()
        .map(|entry| {
            let slices: Vec<_> = entry
                .parts
                .iter()
                .map(|part| output[&PathBuf::from(&part.path)].clone())
                .collect();
            let doc = decode_concatenated(&entry.parts, &slices).unwrap();
            (entry.id, doc)
        })
        .collect()
}
