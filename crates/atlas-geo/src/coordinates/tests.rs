use super::*;

#[test]
fn minute_transcription_preserves_resolution_and_unknown_datum() {
    let point = LonLat::from_ne_minutes(105, 16, 27, 18).unwrap();
    assert!((point.longitude - 105.26666666666667).abs() < 1e-12);
    let observed = CoordinateObservation {
        point,
        longitude_text: "105°16′".into(),
        latitude_text: "27°18′".into(),
        resolution_arcminutes: 1.0,
        datum: Datum::Unspecified,
    };
    assert!(observed.validate(BodyId::Earth).is_ok());
    assert_eq!(observed.datum, Datum::Unspecified);
    assert!(LonLat::from_ne_minutes(105, 60, 27, 18).is_err());
    assert!(
        LonLat {
            longitude: f64::NAN,
            latitude: 0.0
        }
        .validate()
        .is_err()
    );
    let wrong = CoordinateObservation {
        datum: Datum::MoonIau,
        ..observed
    };
    assert!(wrong.validate(BodyId::Earth).is_err());
}
