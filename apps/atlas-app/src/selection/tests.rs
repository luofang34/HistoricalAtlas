use super::*;

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
