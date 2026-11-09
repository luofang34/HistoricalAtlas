use super::*;
use crate::{export::compile, input::load_blocking};
use atlas_geo::{coordinates::Datum, registration::PlanarPoint};
use atlas_ledger::model::EstimateValue;
use std::path::{Path, PathBuf};

#[cfg(test)]
fn input() -> LedgerInput {
    let mut input =
        load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    input.derivations.clear();
    input
}

#[test]
fn bijie_map_uses_four_controls_and_twenty_five_independent_checks_without_changing_source_coordinates()
 {
    let input = input();
    let map = &input.place_maps["bijie-town"];
    let (sheet, estimates) = register_places(&input, map).unwrap();
    assert_eq!(estimates.len(), 29);
    assert_eq!(sheet.control_quality.points, 4);
    let quality = sheet.check_quality.unwrap();
    assert_eq!(quality.points, 25);
    assert!((0.025..0.026).contains(&quality.maximum));
    assert_eq!(
        sheet.job.target_frame.crs,
        "ATLAS:source-angular:Unspecified"
    );
    let daqiao = estimates
        .iter()
        .find(|e| e.key == "bijie-gazetteer/town-map/daqiao-commune/registered-location")
        .unwrap();
    let EstimateValue::MapLocation {
        point,
        datum,
        uncertainty_m,
        ..
    } = daqiao.value
    else {
        panic!("location estimate expected")
    };
    assert!((point.longitude - 105.2765584635).abs() < 1e-8);
    assert!((point.latitude - 27.3188353435).abs() < 1e-8);
    assert_eq!(datum, Datum::Unspecified);
    assert_eq!(uncertainty_m, 4500.0);
    let table = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/p24/daqiao/daqiao-commune")
        .unwrap();
    assert_eq!(
        table
            .observation
            .coordinate
            .as_ref()
            .unwrap()
            .longitude_text,
        "105°16′"
    );
    assert!(daqiao.basis.contains(&table.id));
    assert!(
        estimates
            .iter()
            .all(|e| input.data.estimates.iter().any(|stored| stored.id == e.id))
    );
    let output = compile(&input).unwrap();
    let artifact: serde_json::Value =
        serde_json::from_slice(&output[&PathBuf::from("maps/bijie-town.json")]).unwrap();
    assert_eq!(
        artifact["registration"]["scan_sha256"],
        "3591c54d1e4438ce87241df13858159c07135a4fd1efffe87df4aa6e1fd91bb1"
    );
    assert_eq!(artifact["map"]["places"].as_array().unwrap().len(), 29);
    assert_eq!(
        artifact["registration"]["job"]["source_frame"]["width"],
        1648
    );
}

#[test]
fn map_registration_rejects_wrong_identity_datum_and_reused_holdout() {
    let input = input();
    let original = &input.place_maps["bijie-town"];
    let mut map = original.clone();
    map.controls[0].coordinate_record_id = map.controls[1].coordinate_record_id;
    assert!(
        register_places(&input, &map)
            .unwrap_err()
            .to_string()
            .contains("same-entity")
    );
    let mut map = original.clone();
    map.datum = Datum::Wgs84;
    assert!(
        register_places(&input, &map)
            .unwrap_err()
            .to_string()
            .contains("declared datum")
    );
    let mut map = original.clone();
    map.check_points[0] = map.controls[0].clone();
    assert!(
        register_places(&input, &map)
            .unwrap_err()
            .to_string()
            .contains("independent")
    );
}

#[test]
fn map_registration_rejects_wrong_sheet_duplicate_subject_and_outside_symbol() {
    let input = input();
    let original = &input.place_maps["bijie-town"];
    let mut map = original.clone();
    map.scan_page = Some(23);
    assert!(
        register_places(&input, &map)
            .unwrap_err()
            .to_string()
            .contains("mapped sheet")
    );
    let mut map = original.clone();
    map.places.push(map.places[0].clone());
    assert!(
        register_places(&input, &map)
            .unwrap_err()
            .to_string()
            .contains("unique subject")
    );
    let mut map = original.clone();
    map.places[0].point = PlanarPoint { x: -1.0, y: 1.0 };
    assert!(
        register_places(&input, &map)
            .unwrap_err()
            .to_string()
            .contains("outside")
    );
}

#[test]
fn county_registration_checks_the_whole_extent_against_independent_minute_coordinates() {
    use atlas_geo::registration::{AffineRegistration, Registration};

    let input = input();
    let map = &input.place_maps["bijie-county"];
    let (sheet, estimates) = register_places(&input, map).unwrap();
    assert_eq!(sheet.control_quality.points, 6);
    assert_eq!(sheet.check_quality.unwrap().points, 22);
    assert_eq!(estimates.len(), 31);
    let controls: Vec<_> = sheet.job.controls.iter().map(|c| c.point).collect();
    let model = AffineRegistration::fit(&controls).unwrap();
    let mut maximum = 0.0_f64;
    let mut squared_sum = 0.0;
    for check in &sheet.job.check_points {
        let actual = model.transform(check.point.source).unwrap();
        let expected = check.point.target;
        let dx = (actual.x - expected.x) * 111_195.0 * expected.y.to_radians().cos();
        let dy = (actual.y - expected.y) * 111_195.0;
        let error = dx.hypot(dy);
        assert!(error + 1250.0 < map.uncertainty_m, "{}", check.landmark);
        maximum = maximum.max(error);
        squared_sum += error * error;
    }
    assert!((2000.0..2200.0).contains(&maximum));
    assert!((900.0..1100.0).contains(&(squared_sum / 22.0).sqrt()));
    assert_eq!(map.datum, Datum::Unspecified);
    assert_eq!(map.uncertainty_m, 3500.0);
    for control in map.controls.iter().chain(&map.check_points) {
        let source = input
            .data
            .records
            .iter()
            .find(|r| r.id == control.coordinate_record_id)
            .unwrap();
        let coordinate = source.observation.coordinate.as_ref().unwrap();
        assert_eq!(coordinate.resolution_arcminutes, 1.0);
        assert_eq!(coordinate.datum, Datum::Unspecified);
        assert_eq!(
            source.scan_page,
            Some(
                source
                    .page_or_sheet
                    .as_ref()
                    .unwrap()
                    .parse::<u32>()
                    .unwrap()
                    + 35
            )
        );
        assert!(estimates.iter().all(|e| e.basis.contains(&source.id)));
    }
}

#[test]
fn reference_landmarks_join_table_controls_and_holdouts() {
    let input = crate::registration::with_synthetic_reference(input());
    let mut map = input.place_maps["bijie-town"].clone();
    let (baseline, _) = register_places(&input, &map).unwrap();
    // Re-express the first holdout as a landmark measured on a catalogued reference.
    let holdout = baseline.job.check_points[0].clone();
    map.check_points.remove(0);
    map.reference_check_points
        .push(crate::registration::ReferencedControl {
            basis: Vec::new(),
            reference: "Synthetic reference measurement".into(),
            reference_sha256: Some(crate::registration::SYNTHETIC_REFERENCE.into()),
            uncertainty: Some(0.0005),
            ..holdout
        });
    let (sheet, _) = register_places(&input, &map).unwrap();
    assert_eq!(
        sheet.job.check_points.len(),
        baseline.job.check_points.len()
    );
    let quality = sheet.check_quality.unwrap();
    let before = baseline.check_quality.unwrap();
    assert!((quality.maximum - before.maximum).abs() < 1e-12);
    let mut unlicensed = map.clone();
    unlicensed.reference_check_points[0].reference_sha256 = Some("e".repeat(64));
    assert!(register_places(&input, &unlicensed).is_err());
}
