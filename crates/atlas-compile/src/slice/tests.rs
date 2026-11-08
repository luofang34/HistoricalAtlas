use super::*;

#[test]
fn slice_command_writes_shards_and_checked_evidence_with_original_dates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("slice");
    let at = atlas_core::calendar::CivilDate {
        year: 1966,
        month: 7,
        day: 1,
    }
    .to_epoch(atlas_core::calendar::Calendar::Gregorian, 0)
    .unwrap();
    let count = write_slice_blocking(&root, &at.ticks().to_string(), "nominal", &output).unwrap();
    assert!(count > 2);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("slice.json")).unwrap()).unwrap();
    assert_eq!(manifest["at"], at.ticks().to_string());
    assert_eq!(manifest["selection"], "nominal");
    let shard_name = "4b";
    assert!(
        manifest["shards"]
            .as_array()
            .unwrap()
            .iter()
            .any(|name| name == shard_name)
    );
    let shard: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output.join(format!("slice/index/{shard_name}.json"))).unwrap(),
    )
    .unwrap();
    let subject = "4b13e508-16cc-53f4-a503-9d03642b966f";
    let properties = shard[subject]["properties"].as_array().unwrap();
    let name = properties
        .iter()
        .find(|property| property["property"]["property"] == "name")
        .unwrap();
    assert_eq!(
        name["candidates"][0]["assertion"]["value"]["value"]["text"],
        "城关镇"
    );
    assert_eq!(name["candidates"][0]["membership"], "certain");
    let evidence: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output.join(format!("entities/index/{shard_name}.json"))).unwrap(),
    )
    .unwrap();
    let record = evidence[subject]["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["source_date"]["text"] == "1962年")
        .unwrap();
    assert_eq!(record["scan_page"], 47);
    assert_eq!(record["page_or_sheet"], "12");
    let disputes = evidence[subject]["disputes"].as_array().unwrap();
    assert_eq!(disputes.len(), 2);
    for dispute in disputes {
        let records = evidence[subject]["records"].as_array().unwrap();
        let mut pages: Vec<_> = dispute["alternatives"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| {
                records.iter().find(|record| record["id"] == *id).unwrap()["scan_page"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        pages.sort();
        assert_eq!(pages, [42, 47]);
    }
    assert!(write_slice_blocking(&root, &at.ticks().to_string(), "nominal", &output).is_err());
}

#[test]
fn query_errors_precede_output_and_repeated_compilation_is_identical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("slice");
    assert!(matches!(
        write_slice_blocking(&root, "1.5", "nominal", &output),
        Err(SliceError::Ticks { .. })
    ));
    assert!(matches!(
        write_slice_blocking(&root, "0", "guess", &output),
        Err(SliceError::Selection(_))
    ));
    assert!(!output.exists());
    let input = load_blocking(&root).unwrap();
    let artifacts =
        compile_slice(&input, EpochTime::from_ticks(0), TimeSelection::Possible).unwrap();
    assert_eq!(
        artifacts,
        compile_slice(&input, EpochTime::from_ticks(0), TimeSelection::Possible).unwrap()
    );
    assert!(
        artifacts
            .values()
            .all(|bytes| bytes.len() < crate::export::MAX_FILE_BYTES)
    );
}

#[test]
fn lunar_origins_select_deep_time_and_preserve_unquantified_ages() {
    use atlas_ledger::timeline::Timeline;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    let at = |years| EpochTime::years_before(EpochTime::from_ticks(0), years).unwrap();
    for (age, count) in [
        (4_550_000_000, 0),
        (4_500_000_000, 1),
        (3_900_000_000, 2),
        (100_000_000, 3),
    ] {
        let slice = timeline.at(at(age), TimeSelection::Nominal).unwrap();
        assert_eq!(
            slice
                .entities
                .iter()
                .flat_map(|entity| &entity.origins)
                .count(),
            count
        );
    }
    let possible = timeline
        .at(at(4_550_000_000), TimeSelection::Possible)
        .unwrap();
    assert!(
        possible
            .entities
            .iter()
            .flat_map(|entity| &entity.origins)
            .any(|origin| matches!(
                origin.time,
                atlas_core::origin::OriginTime::NoLaterThan { .. }
            ))
    );
    let certain = timeline
        .at(EpochTime::from_ticks(0), TimeSelection::Certain)
        .unwrap();
    let names: Vec<_> = certain
        .entities
        .iter()
        .flat_map(|entity| &entity.origins)
        .map(|origin| {
            input
                .data
                .estimates
                .iter()
                .find(|estimate| estimate.id == origin.estimate_id)
                .unwrap()
                .key
                .as_str()
        })
        .collect();
    assert_eq!(names.len(), 3);
    assert!(!names.iter().any(|name| name.contains("tycho")));
}

#[test]
fn landing_slice_retains_the_second_disagreement_and_lunar_reference_frame() {
    use atlas_core::{
        calendar::{Calendar, CivilDate},
        origin::OriginTime,
        time::TimeMembership,
    };
    use atlas_ledger::{model::EstimateValue, timeline::Timeline};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let input = load_blocking(&root).unwrap();
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|estimate| estimate.key == "editorial/tranquility-base/establishment")
        .unwrap();
    let EstimateValue::Origin {
        time: OriginTime::Estimated { instant },
    } = &estimate.value
    else {
        panic!("expected landing date")
    };
    let seconds = 20 * 3600 + 17 * 60 + 40;
    let expected = CivilDate {
        year: 1969,
        month: 7,
        day: 20,
    }
    .to_epoch(Calendar::Gregorian, 39_759)
    .unwrap()
    .checked_add(seconds * 1000)
    .unwrap();
    assert_eq!(instant.nominal, expected);
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    let before = timeline
        .at(
            instant.nominal.checked_add(-1).unwrap(),
            TimeSelection::Nominal,
        )
        .unwrap();
    assert!(
        before
            .entities
            .iter()
            .all(|entity| entity.entity_id != estimate.entity_id)
    );
    let slice = timeline
        .at(instant.nominal, TimeSelection::Nominal)
        .unwrap();
    let origin = &slice
        .entities
        .iter()
        .find(|entity| entity.entity_id == estimate.entity_id)
        .unwrap()
        .origins[0];
    assert_eq!(origin.membership, TimeMembership::Possible);
    assert_eq!(origin.disputes.len(), 1);
    assert_eq!(origin.basis.len(), 2);
    let output = compile_slice(&input, instant.latest, TimeSelection::Certain).unwrap();
    let shard = estimate.entity_id.shard();
    let slice: serde_json::Value = serde_json::from_slice(
        &output[&std::path::PathBuf::from(format!("slice/index/{shard}.json"))],
    )
    .unwrap();
    assert_eq!(
        slice[estimate.entity_id.0.to_string()]["origins"][0]["membership"],
        "certain"
    );
    assert_lunar_evidence(&output, estimate.entity_id);
}

#[test]
fn origins_require_checked_subject_evidence_and_moon_coordinates_reject_earth() {
    use atlas_geo::body::BodyId;
    use atlas_ledger::validation::validate;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    let evidence = input
        .data
        .records
        .iter()
        .find(|record| record.source_id == "bijie-gazetteer" && record.checked)
        .unwrap()
        .id;
    let moon = input
        .data
        .estimates
        .iter_mut()
        .find(|estimate| estimate.key == "editorial/moon/formation-limit")
        .unwrap();
    moon.basis = vec![evidence];
    assert!(
        validate(&input.catalog, &input.data)
            .unwrap_err()
            .problem
            .contains("subject")
    );
    let position = input
        .data
        .records
        .iter()
        .find_map(|record| {
            record.observation.coordinate.as_ref().filter(|coordinate| {
                coordinate.datum == atlas_geo::coordinates::Datum::MoonMeanEarthDe421
            })
        })
        .unwrap();
    assert!(position.validate(BodyId::Earth).is_err());
    assert!(position.validate(BodyId::Mars).is_err());
    position.validate(BodyId::Moon).unwrap();
}

#[cfg(test)]
fn assert_lunar_evidence(
    output: &crate::export::Artifacts,
    entity_id: atlas_core::identity::EntityId,
) {
    let shard = entity_id.shard();
    let evidence: serde_json::Value = serde_json::from_slice(
        &output[&std::path::PathBuf::from(format!("entities/index/{shard}.json"))],
    )
    .unwrap();
    let records = evidence[entity_id.0.to_string()]["records"]
        .as_array()
        .unwrap();
    let position = records
        .iter()
        .find(|record| record["source_id"] == "nasa-apollo-coordinates-2017")
        .unwrap();
    assert!(position["scan_page"].is_null());
    assert_eq!(
        position["observation"]["coordinate"]["datum"],
        "moon_mean_earth_de421"
    );
    assert_eq!(
        position["observation"]["coordinate"]["point"]["longitude"],
        23.47314
    );
    let sources: serde_json::Value =
        serde_json::from_slice(&output[&std::path::PathBuf::from("sources.json")]).unwrap();
    assert!(
        sources
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["id"] == "nasa-apollo-coordinates-2017"
                && source["url"].as_str().unwrap().starts_with("https://"))
    );
}
