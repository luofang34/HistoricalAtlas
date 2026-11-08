use super::*;

#[cfg(test)]
fn kangxi() -> EraConcordance {
    EraConcordance {
        dynasty: "清".into(),
        era: "康熙".into(),
        year: 26,
        period: EraPeriod::Month {
            month: 4,
            leap: false,
        },
        calendar: Calendar::Reform1582,
        first_day: CivilDate {
            year: 1687,
            month: 5,
            day: 11,
        },
        last_day: CivilDate {
            year: 1687,
            month: 6,
            day: 9,
        },
        source_id: "academia-sinica-calendar".into(),
        locator: "Qing, Kangxi year 26, ordinary month 4; reform 1582-10-15".into(),
    }
}

#[test]
fn fourth_lunar_month_preserves_thirty_days_and_calendar_offset() {
    let date = kangxi();
    let result = date.to_estimate(0).unwrap();
    assert_eq!(result.earliest.ticks(), -9_866_059_200_000);
    assert_eq!(result.latest.ticks(), -9_863_467_200_001);
    assert_eq!(
        result.latest.ticks() - result.earliest.ticks() + 1,
        30 * DAY_MS
    );
    assert_eq!(result.nominal.ticks(), -9_864_763_200_001);
    assert_eq!(result.precision, TimePrecision::Month);
    let shifted = date.to_estimate(12_345).unwrap();
    assert_eq!(shifted.earliest.ticks() - result.earliest.ticks(), 12_345);
    assert_eq!(shifted.latest.ticks() - result.latest.ticks(), 12_345);
    let roundtrip: EraConcordance =
        serde_json::from_str(&serde_json::to_string(&date).unwrap()).unwrap();
    assert_eq!(roundtrip, date);
}

#[test]
fn cited_era_year_crosses_a_western_year_in_the_declared_calendar() {
    let mut date = kangxi();
    date.dynasty = "唐".into();
    date.era = "總章".into();
    date.year = 2;
    date.period = EraPeriod::Year;
    date.first_day = CivilDate {
        year: 669,
        month: 2,
        day: 6,
    };
    date.last_day = CivilDate {
        year: 670,
        month: 1,
        day: 26,
    };
    let result = date.to_estimate(0).unwrap();
    let gregorian = CivilDate {
        year: 669,
        month: 2,
        day: 9,
    };
    assert_eq!(
        result.earliest,
        gregorian.to_epoch(Calendar::Gregorian, 0).unwrap()
    );
    assert_eq!(
        result.latest.ticks() - result.earliest.ticks() + 1,
        355 * DAY_MS
    );
    assert_eq!(result.precision, TimePrecision::Year);
}

#[test]
fn malformed_era_notation_and_citations_fail() {
    let date = kangxi();
    for period in [
        EraPeriod::Month {
            month: 0,
            leap: false,
        },
        EraPeriod::Day {
            month: 13,
            leap: true,
            day: 1,
        },
        EraPeriod::Day {
            month: 1,
            leap: false,
            day: 31,
        },
    ] {
        assert!(matches!(
            EraConcordance {
                period,
                ..date.clone()
            }
            .to_estimate(0),
            Err(EraError::InvalidPeriod(_))
        ));
    }
    assert!(matches!(
        EraConcordance {
            year: 0,
            ..date.clone()
        }
        .to_estimate(0),
        Err(EraError::InvalidEra { .. })
    ));
    assert!(matches!(
        EraConcordance {
            locator: String::new(),
            ..date.clone()
        }
        .to_estimate(0),
        Err(EraError::MissingCitation)
    ));
}

#[test]
fn malformed_civil_bounds_and_time_overflows_fail() {
    let date = kangxi();
    let mut inverted = date.clone();
    std::mem::swap(&mut inverted.first_day, &mut inverted.last_day);
    assert!(matches!(
        inverted.to_estimate(0),
        Err(EraError::InvalidBounds { .. })
    ));
    let long_month = EraConcordance {
        last_day: CivilDate {
            day: 10,
            ..date.last_day
        },
        ..date.clone()
    };
    assert!(matches!(
        long_month.to_estimate(0),
        Err(EraError::InvalidBounds { .. })
    ));
    let invalid = EraConcordance {
        first_day: CivilDate {
            month: 2,
            day: 30,
            ..date.first_day
        },
        ..date.clone()
    };
    assert!(matches!(invalid.to_estimate(0), Err(EraError::Calendar(_))));
    assert!(matches!(
        date.to_estimate(i128::MIN),
        Err(EraError::Time(_))
    ));
}

#[test]
fn day_precision_and_intercalary_month_identity_survive_conversion() {
    let mut date = kangxi();
    date.period = EraPeriod::Day {
        month: 4,
        leap: false,
        day: 1,
    };
    date.last_day = date.first_day;
    let result = date.to_estimate(0).unwrap();
    assert_eq!(result.precision, TimePrecision::Day);
    assert_eq!(result.latest.ticks() - result.earliest.ticks() + 1, DAY_MS);
    let period = EraPeriod::Month {
        month: 4,
        leap: true,
    };
    let encoded = serde_json::to_value(period).unwrap();
    assert_eq!(encoded["leap"], true);
    assert_eq!(
        serde_json::from_value::<EraPeriod>(encoded).unwrap(),
        period
    );
}
