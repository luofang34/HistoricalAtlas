use super::*;
use crate::time::JULIAN_YEAR_MS;

#[test]
fn deep_time_constraints_keep_one_sided_and_unquantified_uncertainty() {
    let reference = EpochTime::from_ticks(0);
    let instant = InstantEstimate {
        earliest: EpochTime::years_before(reference, 4_520_000_000).unwrap(),
        nominal: EpochTime::years_before(reference, 4_510_000_000).unwrap(),
        latest: EpochTime::years_before(reference, 4_500_000_000).unwrap(),
        precision: TimePrecision::Ga,
    };
    let dated = OriginTime::Estimated { instant };
    let limit = OriginTime::NoLaterThan { limit: instant };
    let earlier = instant.earliest.checked_add(-1).unwrap();
    assert_eq!(dated.occurred_by(earlier).unwrap(), TimeMembership::Outside);
    assert_eq!(
        limit.occurred_by(earlier).unwrap(),
        TimeMembership::Possible
    );
    assert_eq!(
        limit.occurred_by(instant.latest).unwrap(),
        TimeMembership::Certain
    );
    assert_eq!(
        dated.occurred_by(instant.nominal).unwrap(),
        TimeMembership::Possible
    );
    assert_eq!(dated.nominal().ticks(), -4_510_000_000 * JULIAN_YEAR_MS);
    let approximate = OriginTime::Approximate {
        nominal: instant.nominal,
        precision: TimePrecision::Ga,
    };
    assert_eq!(
        approximate.occurred_by(reference).unwrap(),
        TimeMembership::Possible
    );
    assert_eq!(
        approximate.occurred_by(earlier).unwrap(),
        TimeMembership::Possible
    );
    let json = serde_json::to_string(&limit).unwrap();
    assert_eq!(serde_json::from_str::<OriginTime>(&json).unwrap(), limit);
}

#[test]
fn bounded_formation_includes_exact_latest_and_rejects_inverted_ranges() {
    let instant = InstantEstimate {
        earliest: EpochTime::from_ticks(10),
        nominal: EpochTime::from_ticks(20),
        latest: EpochTime::from_ticks(30),
        precision: TimePrecision::Millisecond,
    };
    let dated = OriginTime::Estimated { instant };
    assert_eq!(
        dated.occurred_by(EpochTime::from_ticks(29)).unwrap(),
        TimeMembership::Possible
    );
    assert_eq!(
        dated.occurred_by(EpochTime::from_ticks(30)).unwrap(),
        TimeMembership::Certain
    );
    let invalid = InstantEstimate {
        nominal: EpochTime::from_ticks(31),
        ..instant
    };
    for value in [
        OriginTime::Estimated { instant: invalid },
        OriginTime::NoLaterThan { limit: invalid },
    ] {
        assert!(value.occurred_by(EpochTime::from_ticks(100)).is_err());
    }
}
