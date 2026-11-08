use super::*;
use crate::selection::{SelectedTime, TimeControl, TimeMode};
use atlas_core::time::{InstantEstimate, TimePrecision};
use serde_json::{Value, json};

#[cfg(test)]
pub(crate) fn example() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/overlays/apollo11.overlay.json"
    ))
}

#[cfg(test)]
fn at(atlas: &Atlas, tick: i128, selection: TimeSelection) -> SelectedTime {
    SelectedTime::query(
        atlas,
        &TimeControl {
            mode: TimeMode::Ticks,
            value: tick.to_string(),
            selection,
        },
    )
    .unwrap()
}

#[test]
fn external_landing_uses_reviewed_lunar_coordinates_and_exclusive_time_bounds() {
    let atlas = crate::catalog::tests::atlas();
    let counts = (atlas.data.records.len(), atlas.data.estimates.len());
    let overlay = Overlay::parse(
        &atlas,
        "https://events.example/apollo.json".into(),
        example(),
    )
    .unwrap();
    let event = &overlay.package().events[0];
    let nominal = event.focus_time().unwrap().unwrap().ticks();
    let time = at(&atlas, nominal, TimeSelection::Nominal);
    let features = overlay.features(&atlas, BodyId::Moon, &time, None).unwrap();
    assert_eq!(features.len(), 1);
    assert_eq!(
        features[0]["geometry"]["coordinates"],
        json!([23.47314, 0.67416])
    );
    assert_eq!(features[0]["properties"]["datum"], "moon_mean_earth_de421");
    assert_eq!(features[0]["properties"]["resolution_arcminutes"], 0.0006);
    assert_eq!(
        features[0]["properties"]["event_id"],
        "apollo-11-lunar-landing"
    );
    assert!(
        overlay
            .features(&atlas, BodyId::Earth, &time, None)
            .unwrap()
            .is_empty()
    );
    for tick in [nominal - 1, nominal + 1] {
        assert!(
            overlay
                .features(
                    &atlas,
                    BodyId::Moon,
                    &at(&atlas, tick, TimeSelection::Nominal),
                    None
                )
                .unwrap()
                .is_empty()
        );
    }
    let origin = atlas
        .data
        .estimates
        .iter()
        .find(|value| value.key == "editorial/tranquility-base/establishment")
        .unwrap();
    let atlas_ledger::model::EstimateValue::Origin {
        time: atlas_core::origin::OriginTime::Estimated { instant },
    } = origin.value
    else {
        panic!("expected landing estimate")
    };
    assert_eq!(event.validity.start, Some(instant));
    assert_eq!(
        counts,
        (atlas.data.records.len(), atlas.data.estimates.len())
    );
}

#[test]
fn package_references_resolve_aliases_without_rewriting_external_ids() {
    let mut publication = crate::catalog::tests::publication();
    let target = EntityId::from_key("moon/tranquility-base").unwrap();
    let old = EntityId::from_key("moon/landing-site-alias").unwrap();
    let intermediate = EntityId::from_key("moon/landing-site-intermediate").unwrap();
    publication.aliases.0.insert(old, intermediate);
    publication.aliases.0.insert(intermediate, target);
    let atlas = Atlas::new(publication).unwrap();
    let mut document: Value = serde_json::from_str(example()).unwrap();
    document["events"][0]["entities"][0] = json!(old);
    let overlay = Overlay::parse(&atlas, "aliased.json".into(), &document.to_string()).unwrap();
    assert_eq!(atlas.search(&old.0.to_string(), BodyId::Moon), vec![target]);
    let event = &overlay.package().events[0];
    let time = at(
        &atlas,
        event.focus_time().unwrap().unwrap().ticks(),
        TimeSelection::Nominal,
    );
    let features = overlay
        .features(&atlas, BodyId::Moon, &time, Some(target))
        .unwrap();
    assert_eq!(features[0]["properties"]["anchor_id"], json!(old));
    assert_eq!(features[0]["properties"]["entity_id"], json!(target));
    assert_eq!(overlay.package().events[0].entities, vec![old]);
    assert_eq!(event.anchors(&atlas, BodyId::Moon), vec![target]);
    document["events"][0]["entities"]
        .as_array_mut()
        .unwrap()
        .push(json!(target));
    assert!(matches!(
        Overlay::parse(&atlas, "repeated-alias.json".into(), &document.to_string()),
        Err(OverlayError::RepeatedAnchor { .. })
    ));
}

#[test]
fn coincident_event_anchors_keep_their_identities_and_unlocated_events_keep_their_links() {
    let atlas = crate::catalog::tests::atlas();
    let mut document: Value = serde_json::from_str(example()).unwrap();
    let find = |name: &str| {
        atlas
            .search(name, BodyId::Earth)
            .into_iter()
            .find(|id| atlas.label(*id) == name)
            .unwrap()
    };
    let first = find("厦耳");
    let second = find("崩土山");
    document["events"][0]["entities"] = json!([first, second]);
    let overlay = Overlay::parse(&atlas, "coincident.json".into(), &document.to_string()).unwrap();
    let time = at(&atlas, -960_910_900_241, TimeSelection::Nominal);
    let features = overlay
        .features(&atlas, BodyId::Earth, &time, None)
        .unwrap();
    assert_eq!(features.len(), 2);
    assert_eq!(features[0]["geometry"], features[1]["geometry"]);
    assert_ne!(
        features[0]["properties"]["entity_id"],
        features[1]["properties"]["entity_id"]
    );
    let county = EntityId::from_key("earth/cn-gz/bijie/bijie-county").unwrap();
    document["events"][0]["entities"] = json!([county]);
    let overlay = Overlay::parse(&atlas, "unlocated.json".into(), &document.to_string()).unwrap();
    assert!(
        overlay
            .features(&atlas, BodyId::Earth, &time, None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        overlay.package().events[0].anchors(&atlas, BodyId::Earth),
        vec![county]
    );
}

#[test]
fn malformed_packages_fail_before_rendering() {
    let atlas = crate::catalog::tests::atlas();
    assert!(matches!(
        Overlay::parse(&atlas, "large.json".into(), &"x".repeat(4_000_000)),
        Err(OverlayError::Oversize(4_000_000))
    ));
    let base: Value = serde_json::from_str(example()).unwrap();
    for (pointer, value) in [
        ("/version", json!(2)),
        ("/events/0/id", json!("")),
        ("/events/0/entities", json!([])),
        (
            "/events/0/entities/0",
            json!(EntityId::from_key("earth/missing").unwrap()),
        ),
        ("/events/0/url", json!("javascript:alert(1)")),
        ("/events/0/validity/end/nominal", json!("-960910999999")),
        (
            "/events/0/validity/start/earliest",
            json!(-960910901741_i64),
        ),
    ] {
        let mut document = base.clone();
        *document.pointer_mut(pointer).unwrap() = value;
        assert!(
            Overlay::parse(&atlas, "invalid.json".into(), &document.to_string()).is_err(),
            "{pointer}"
        );
    }
    let mut duplicate = base.clone();
    duplicate["events"]
        .as_array_mut()
        .unwrap()
        .push(base["events"][0].clone());
    assert!(matches!(
        Overlay::parse(&atlas, "duplicate.json".into(), &duplicate.to_string()),
        Err(OverlayError::EventId(_))
    ));
    let mut repeated = base.clone();
    repeated["events"][0]["entities"]
        .as_array_mut()
        .unwrap()
        .push(base["events"][0]["entities"][0].clone());
    assert!(matches!(
        Overlay::parse(&atlas, "repeated.json".into(), &repeated.to_string()),
        Err(OverlayError::RepeatedAnchor { .. })
    ));
}

#[test]
fn deep_time_event_selection_retains_endpoint_uncertainty_and_undated_state() {
    let start = -142_324_776_000_000_000_000_i128;
    let instant = |offset| InstantEstimate {
        earliest: EpochTime::from_ticks(start + offset),
        nominal: EpochTime::from_ticks(start + offset + 10),
        latest: EpochTime::from_ticks(start + offset + 20),
        precision: TimePrecision::Millisecond,
    };
    let mut package: OverlayPackage = serde_json::from_str(example()).unwrap();
    let event = &mut package.events[0];
    event.validity = TemporalSpan {
        start: Some(instant(0)),
        end: Some(instant(100)),
    };
    for (offset, nominal, possible, certain) in [
        (-1, false, false, false),
        (0, false, true, false),
        (10, true, true, false),
        (20, true, true, true),
        (100, true, true, false),
        (110, false, true, false),
        (120, false, false, false),
    ] {
        let at = EpochTime::from_ticks(start + offset);
        assert_eq!(event.matches(at, TimeSelection::Nominal).unwrap(), nominal);
        assert_eq!(
            event.matches(at, TimeSelection::Possible).unwrap(),
            possible
        );
        assert_eq!(event.matches(at, TimeSelection::Certain).unwrap(), certain);
    }
    event.validity = TemporalSpan {
        start: None,
        end: None,
    };
    assert!(event.focus_time().unwrap().is_none());
    for selection in [
        TimeSelection::Nominal,
        TimeSelection::Possible,
        TimeSelection::Certain,
    ] {
        assert!(
            !event
                .matches(EpochTime::from_ticks(start), selection)
                .unwrap()
        );
    }
}
