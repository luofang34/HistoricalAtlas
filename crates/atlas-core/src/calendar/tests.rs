use super::*;

#[test]
fn reform_crosses_one_physical_day() {
    let before = CivilDate {
        year: 1582,
        month: 10,
        day: 4,
    };
    let after = CivilDate { day: 15, ..before };
    assert_eq!(
        after.to_epoch(Calendar::Reform1582, 0).unwrap().ticks()
            - before.to_epoch(Calendar::Reform1582, 0).unwrap().ticks(),
        86_400_000
    );
    for day in 5..15 {
        assert!(
            CivilDate { day, ..before }
                .to_epoch(Calendar::Reform1582, 0)
                .is_err()
        );
    }
}

#[test]
fn j2000_and_utc_offsets_are_explicit() {
    let date = CivilDate {
        year: 2000,
        month: 1,
        day: 1,
    };
    let tt = date.to_epoch(Calendar::Gregorian, 0).unwrap();
    let utc = date.to_epoch(Calendar::Gregorian, 64_184).unwrap();
    assert_eq!(tt.ticks(), -43_200_000);
    assert_eq!(utc.ticks() - tt.ticks(), 64_184);
}

#[test]
fn calendar_leaps_and_extreme_years_are_validated() {
    let date = CivilDate {
        year: 1900,
        month: 2,
        day: 29,
    };
    assert!(date.to_epoch(Calendar::Gregorian, 0).is_err());
    assert!(date.to_epoch(Calendar::Julian, 0).is_ok());
    for year in [0, -4801, -1_000_000_000_000_000, 1_000_000_000_000_000] {
        assert!(
            CivilDate {
                year,
                month: 1,
                day: 1
            }
            .to_epoch(Calendar::Gregorian, 0)
            .is_ok()
        );
    }
}
