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
    let subject_entry = load_disk_entity(&output, subject);
    let record = subject_entry["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["source_date"]["text"] == "1962年")
        .unwrap();
    assert_eq!(record["scan_page"], 47);
    assert_eq!(record["page_or_sheet"], "12");
    let disputes = subject_entry["disputes"].as_array().unwrap();
    assert_eq!(disputes.len(), 2);
    for dispute in disputes {
        let records = subject_entry["records"].as_array().unwrap();
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

#[cfg(test)]
fn load_disk_entity(output: &Path, subject: &str) -> serde_json::Value {
    let root_manifest: atlas_boundary::publication::PublicationManifest =
        serde_json::from_slice(&std::fs::read(output.join("manifest.json")).unwrap()).unwrap();
    root_manifest.validate().unwrap();
    for cat in &root_manifest.catalog_shards {
        let items: Vec<atlas_boundary::publication::CatalogItem> =
            serde_json::from_slice(&std::fs::read(output.join(&cat.part.path)).unwrap()).unwrap();
        if let Some(item) = items.iter().find(|item| item.id.0.to_string() == subject) {
            let shard_map: serde_json::Value = serde_json::from_slice(
                &std::fs::read(output.join(format!("entities/index/{}.json", item.shard))).unwrap(),
            )
            .unwrap();
            return shard_map[subject].clone();
        }
    }
    panic!("missing entity {subject}");
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
    let mut input = load_blocking(&root).unwrap();
    input.derivations.clear();
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
    let mut input = load_blocking(&root).unwrap();
    input.derivations.clear();
    let lunar_ids: std::collections::BTreeSet<_> = input
        .data
        .entities
        .iter()
        .filter(|entity| entity.body == atlas_geo::body::BodyId::Moon)
        .map(|entity| entity.id)
        .collect();
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
                .filter(|entity| lunar_ids.contains(&entity.entity_id))
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
        .filter(|entity| lunar_ids.contains(&entity.entity_id))
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
fn commune_and_cemetery_origins_follow_source_year_and_month_precision() {
    use atlas_core::time::TimePrecision;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    input.derivations.clear();
    for commune in ["daqiao", "dalan", "dehou"] {
        assert_establishment_window(
            &input,
            &format!("editorial/bijie/{commune}/establishment"),
            month_start(1958, 1),
            month_start(1959, 1),
            TimePrecision::Year,
        );
    }
    assert_establishment_window(
        &input,
        "editorial/bijie/daqiao/lieshi-lingyuan/establishment",
        month_start(1958, 4),
        month_start(1958, 5),
        TimePrecision::Month,
    );
}

#[cfg(test)]
fn month_start(year: i64, month: u8) -> EpochTime {
    atlas_core::calendar::CivilDate {
        year,
        month,
        day: 1,
    }
    .to_epoch(atlas_core::calendar::Calendar::Gregorian, 0)
    .unwrap()
}

#[cfg(test)]
fn assert_establishment_window(
    input: &LedgerInput,
    key: &str,
    earliest: EpochTime,
    end_exclusive: EpochTime,
    precision: atlas_core::time::TimePrecision,
) {
    use atlas_core::origin::OriginTime;
    use atlas_ledger::model::EstimateValue;
    let timeline = Timeline::new(&input.catalog, &input.data).unwrap();
    let estimate = input
        .data
        .estimates
        .iter()
        .find(|estimate| estimate.key == key)
        .unwrap();
    let EstimateValue::Origin {
        time: OriginTime::Estimated { instant },
    } = &estimate.value
    else {
        panic!("expected bounded establishment date");
    };
    let latest = end_exclusive.checked_add(-1).unwrap();
    assert_eq!(instant.earliest, earliest);
    assert_eq!(instant.latest, latest);
    assert_eq!(instant.precision, precision);
    let before = timeline
        .at(earliest.checked_add(-1).unwrap(), TimeSelection::Possible)
        .unwrap();
    assert!(
        !before
            .entities
            .iter()
            .any(|entity| entity.entity_id == estimate.entity_id)
    );
    let during = timeline
        .at(instant.nominal, TimeSelection::Certain)
        .unwrap();
    assert!(
        !during
            .entities
            .iter()
            .any(|entity| entity.entity_id == estimate.entity_id)
    );
    let after = compile_slice(input, latest, TimeSelection::Certain).unwrap();
    let shard =
        std::path::PathBuf::from(format!("slice/index/{}.json", estimate.entity_id.shard()));
    let value: serde_json::Value = serde_json::from_slice(&after[&shard]).unwrap();
    assert_eq!(
        value[estimate.entity_id.0.to_string()]["origins"][0]["membership"],
        "certain"
    );
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
    let mut input = load_blocking(&root).unwrap();
    input.derivations.clear();
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
    input.derivations.clear();
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
    let entry = crate::export::lookup_entity(output, entity_id)
        .unwrap()
        .unwrap();
    let records = entry["records"].as_array().unwrap();
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
    let sources = crate::export::load_sources(output).unwrap();
    assert!(sources.iter().any(|source| {
        source.id == "nasa-apollo-coordinates-2017"
            && source.url.as_deref().unwrap().starts_with("https://")
    }));
}

#[test]
fn deep_time_slices_publish_plate_rotated_positions_matching_reconstruction_jobs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut input = load_blocking(&root).unwrap();
    input.derivations.clear();
    let job: crate::reconstruction::ReconstructionJob = crate::input::read_json_blocking(
        &root.join("paleogeography/panxian-240ma.reconstruction.json"),
    )
    .unwrap();
    let expected = crate::reconstruction::reconstruct(&input, job.clone()).unwrap();
    let artifacts = compile_slice(&input, job.at, TimeSelection::Nominal).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&artifacts[Path::new("slice.json")]).unwrap();
    assert_eq!(manifest["paleo"], "slice/paleo.json");
    let positions: Vec<atlas_ledger::plates::PaleoPosition> =
        serde_json::from_slice(&artifacts[Path::new("slice/paleo.json")]).unwrap();
    let anchor = &expected.anchors[0];
    let position = positions
        .iter()
        .find(|position| position.record_id == anchor.coordinate_record)
        .unwrap();
    assert!((position.reconstructed.longitude - anchor.reconstructed.longitude).abs() < 1e-9);
    assert!((position.reconstructed.latitude - anchor.reconstructed.latitude).abs() < 1e-9);
    // At the model's present the rotation is the identity.
    let present = compile_slice(&input, EpochTime::from_ticks(0), TimeSelection::Nominal).unwrap();
    let positions: Vec<atlas_ledger::plates::PaleoPosition> =
        serde_json::from_slice(&present[Path::new("slice/paleo.json")]).unwrap();
    assert!(positions.iter().all(|position| {
        (position.reconstructed.longitude - position.observed.longitude).abs() < 1e-6
            && (position.reconstructed.latitude - position.observed.latitude).abs() < 1e-6
    }));
    // An entity with an origin constraint (e.g. an impact crater) is excluded before formation.
    let panxian =
        atlas_core::identity::EntityId::from_key("earth/cn-gz/panxian/yangjuan-fauna").unwrap();
    let basis = expected.anchors[0].coordinate_record;
    let impact = EpochTime::years_before(EpochTime::from_ticks(0), 66_000_000).unwrap();
    let origin_key = "interpretation/test/impact-origin";
    input.data.estimates.push(
        serde_json::from_value(serde_json::json!({
            "id": atlas_core::identity::FactId::from_key(origin_key).unwrap(),
            "key": origin_key,
            "entity_id": panxian,
            "basis": [basis],
            "brief": "Impact formation age",
            "value": {"kind": "origin", "time": {"constraint": "estimated", "instant": {
                "earliest": impact.ticks().to_string(),
                "nominal": impact.ticks().to_string(),
                "latest": impact.ticks().to_string(),
                "precision": "year"
            }}}
        }))
        .unwrap(),
    );
    let before = compile_slice(&input, job.at, TimeSelection::Nominal).unwrap();
    assert!(!before.contains_key(Path::new("slice/paleo.json")));
    let after = compile_slice(&input, impact, TimeSelection::Nominal).unwrap();
    assert!(after.contains_key(Path::new("slice/paleo.json")));
}
