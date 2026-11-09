//! Complete publication round trip, independent of region names and row counts.

use atlas_compile::{
    export::{MAX_FILE_BYTES, compile},
    input::load_blocking,
};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

#[test]
fn every_checked_assertion_and_geometry_survives_publication() {
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let output = compile(&input).unwrap();
    assert!(output.values().all(|bytes| bytes.len() < MAX_FILE_BYTES));
    let mut entities: BTreeMap<String, Value> = output
        .iter()
        .filter(|(path, _)| path.starts_with("entities/index"))
        .flat_map(|(_, bytes)| serde_json::from_slice::<BTreeMap<String, Value>>(bytes).unwrap())
        .collect();
    for entry in entities.values_mut() {
        let geometry = atlas_compile::export::resolve_geometry(&output, entry).unwrap();
        entry
            .as_object_mut()
            .unwrap()
            .extend(geometry.as_object().unwrap().clone());
    }
    let records: BTreeMap<_, _> = entities
        .values()
        .flat_map(|entity| entity["records"].as_array().unwrap())
        .map(|record| (record["id"].as_str().unwrap(), record))
        .collect();
    for record in &input.data.records {
        if record.checked {
            assert_eq!(
                *records[record.id.0.to_string().as_str()],
                serde_json::to_value(record).unwrap(),
                "{}",
                record.key
            );
        } else {
            assert!(
                !records.contains_key(record.id.0.to_string().as_str()),
                "{}",
                record.key
            );
        }
    }
    let estimates: BTreeMap<_, _> = entities
        .values()
        .flat_map(|entity| entity["estimates"].as_array().unwrap())
        .map(|estimate| (estimate["id"].as_str().unwrap(), estimate))
        .collect();
    for estimate in &input.data.estimates {
        assert_eq!(
            *estimates[estimate.id.0.to_string().as_str()],
            serde_json::to_value(estimate).unwrap(),
            "{}",
            estimate.key
        );
    }
    let derived = entities
        .values()
        .flat_map(|entity| {
            entity["derived_boundaries"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .count();
    assert_eq!(
        derived,
        input
            .derivations
            .values()
            .map(|recipe| recipe.units.len())
            .sum::<usize>()
    );
    let drawings = entities
        .values()
        .flat_map(|entity| entity["drawings"].as_array().into_iter().flatten())
        .count();
    assert_eq!(drawings, input.traces.len());
}
