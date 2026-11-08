use super::*;

#[test]
fn deep_time_roundtrips_without_javascript_number_loss() {
    for years in [4_510_000_000, 1_000_000_000_000_000, -1_000_000_000_000_000] {
        let epoch = EpochTime::years_before(EpochTime::from_ticks(0), years).unwrap();
        let text = serde_json::to_string(&epoch).unwrap();
        assert!(text.starts_with('"'));
        assert_eq!(serde_json::from_str::<EpochTime>(&text).unwrap(), epoch);
        assert_eq!(epoch.checked_add(1).unwrap().ticks() - epoch.ticks(), 1);
    }
    assert!(EpochTime::years_before(EpochTime::from_ticks(0), i128::MAX).is_err());
    assert!(EpochTime::from_ticks(i128::MAX).checked_add(1).is_err());
}

#[test]
fn bp_reference_is_distinct_from_j2000() {
    let bp = EpochTime::from_ticks(-50 * JULIAN_YEAR_MS);
    let age = EpochTime::years_before(bp, 240_000_000).unwrap();
    assert_eq!(age.ticks(), -240_000_050 * JULIAN_YEAR_MS);
}

#[test]
fn imported_uncertainty_must_be_ordered() {
    let estimate = InstantEstimate {
        earliest: EpochTime::from_ticks(1),
        nominal: EpochTime::from_ticks(0),
        latest: EpochTime::from_ticks(2),
        precision: TimePrecision::Year,
    };
    assert!(estimate.validate().is_err());
    let exact = InstantEstimate {
        earliest: estimate.nominal,
        ..estimate
    };
    assert!(exact.validate().is_ok());
    assert!(
        TemporalSpan {
            start: Some(exact),
            end: Some(exact)
        }
        .validate()
        .is_err()
    );
}

#[cfg(test)]
fn bounds(first: i128, nominal: i128, last: i128) -> InstantEstimate {
    InstantEstimate {
        earliest: EpochTime::from_ticks(first),
        nominal: EpochTime::from_ticks(nominal),
        latest: EpochTime::from_ticks(last),
        precision: TimePrecision::Year,
    }
}

#[test]
fn uncertain_boundaries_distinguish_possible_certain_and_excluded_times() {
    let span = TemporalSpan {
        start: Some(bounds(10, 15, 20)),
        end: Some(bounds(30, 35, 40)),
    };
    for (ticks, expected) in [
        (9, TimeMembership::Outside),
        (10, TimeMembership::Possible),
        (19, TimeMembership::Possible),
        (20, TimeMembership::Certain),
        (29, TimeMembership::Certain),
        (30, TimeMembership::Possible),
        (39, TimeMembership::Possible),
        (40, TimeMembership::Outside),
    ] {
        assert_eq!(
            span.membership(EpochTime::from_ticks(ticks)).unwrap(),
            expected
        );
    }
    assert!(!span.nominally_contains(EpochTime::from_ticks(14)).unwrap());
    assert!(span.nominally_contains(EpochTime::from_ticks(15)).unwrap());
    assert!(!span.nominally_contains(EpochTime::from_ticks(35)).unwrap());
}

#[test]
fn unknown_endpoints_remain_uncertain_and_undated_values_have_no_nominal_slice() {
    let mut span = TemporalSpan {
        start: Some(bounds(10, 10, 10)),
        end: None,
    };
    assert_eq!(
        span.membership(EpochTime::from_ticks(100)).unwrap(),
        TimeMembership::Possible
    );
    span.start = None;
    assert_eq!(
        span.membership(EpochTime::from_ticks(100)).unwrap(),
        TimeMembership::Undated
    );
    assert!(!span.nominally_contains(EpochTime::from_ticks(100)).unwrap());
    span.end = Some(bounds(20, 20, 20));
    assert_eq!(
        span.membership(EpochTime::from_ticks(19)).unwrap(),
        TimeMembership::Possible
    );
    assert_eq!(
        span.membership(EpochTime::from_ticks(20)).unwrap(),
        TimeMembership::Outside
    );
}

#[test]
fn deep_time_membership_retains_the_exclusive_millisecond_boundary() {
    let start = EpochTime::years_before(EpochTime::from_ticks(0), 4_510_000_000).unwrap();
    let end = start.checked_add(2).unwrap();
    let span = TemporalSpan {
        start: Some(bounds(start.ticks(), start.ticks(), start.ticks())),
        end: Some(bounds(end.ticks(), end.ticks(), end.ticks())),
    };
    assert_eq!(
        span.membership(start.checked_add(1).unwrap()).unwrap(),
        TimeMembership::Certain
    );
    assert_eq!(span.membership(end).unwrap(), TimeMembership::Outside);
}
