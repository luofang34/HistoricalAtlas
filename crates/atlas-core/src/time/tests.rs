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
