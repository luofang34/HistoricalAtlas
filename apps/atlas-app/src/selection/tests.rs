use super::*;

#[test]
fn campaign_dates_select_days_before_the_gazetteer_period() {
    let mut control = TimeControl {
        mode: TimeMode::Date,
        value: "1942-08-07".into(),
        ..Default::default()
    };
    let first = control.epoch().unwrap();
    control.value = "1942-09-01".into();
    assert_eq!(
        control.epoch().unwrap().ticks() - first.ticks(),
        25 * 86_400_000
    );
    control.value = "1942-02-29".into();
    assert!(control.epoch().is_err());
    control.value = "1942-08".into();
    assert!(control.epoch().is_err());
    control.value = "-0044-03-15".into();
    assert_eq!(
        control.epoch().unwrap(),
        CivilDate {
            year: -44,
            month: 3,
            day: 15
        }
        .to_epoch(Calendar::Gregorian, 0)
        .unwrap()
    );
}

#[test]
fn exact_deep_time_and_millisecond_inputs() {
    let mut control = TimeControl {
        mode: TimeMode::MillionYears,
        value: "4510".into(),
        selection: TimeSelection::Nominal,
    };
    assert_eq!(
        control.epoch().unwrap().ticks(),
        -4_510_000_000 * atlas_core::time::JULIAN_YEAR_MS
    );
    control.mode = TimeMode::Ticks;
    control.value = "-142327776000000000001".into();
    assert_eq!(control.epoch().unwrap().ticks(), -142327776000000000001);
    control.mode = TimeMode::MillionYears;
    control.value = i128::MAX.to_string();
    assert!(control.epoch().is_err());
}

#[test]
fn timeline_controls_reveal_bijie_names_at_the_selected_time() {
    let atlas = crate::catalog::tests::atlas();
    let id = atlas.search("毕节镇", atlas_geo::body::BodyId::Earth)[0];
    for (year, name) in [
        (1955, "城关区"),
        (1960, "灯塔人民公社"),
        (1966, "城关镇"),
        (1982, "毕节镇"),
    ] {
        let time = SelectedTime::query(
            &atlas,
            &TimeControl {
                mode: TimeMode::Year,
                value: year.to_string(),
                selection: TimeSelection::Nominal,
            },
        )
        .unwrap();
        assert!(time.names[&id].contains(&name.to_string()));
    }
}
