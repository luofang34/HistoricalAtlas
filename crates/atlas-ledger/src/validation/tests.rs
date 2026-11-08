use super::*;

#[test]
fn territory_descriptions_require_positive_area_and_distinct_linked_neighbors() {
    use crate::model::territory::{NeighborObservation, TerritoryObservation};
    let mut data: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/bijie/page-20.dataset.json"
    ))
    .unwrap();
    let neighbor = data
        .entities
        .iter()
        .find(|e| e.id != data.records[1].entity_id)
        .unwrap()
        .id;
    data.records[1].observation.territory = Some(TerritoryObservation {
        area_square_km: 12.5,
        neighbors: vec![NeighborObservation {
            name: "Named neighbor".into(),
            entity_id: Some(neighbor),
        }],
    });
    validate(&catalog(), &data).unwrap();
    let mut invalid = data.clone();
    invalid.records[1]
        .observation
        .territory
        .as_mut()
        .unwrap()
        .area_square_km = 0.0;
    assert!(
        validate(&catalog(), &invalid)
            .unwrap_err()
            .problem
            .contains("positive")
    );
    let mut invalid = data.clone();
    let territory = invalid.records[1].observation.territory.as_mut().unwrap();
    territory.neighbors.push(territory.neighbors[0].clone());
    assert!(
        validate(&catalog(), &invalid)
            .unwrap_err()
            .problem
            .contains("distinct")
    );
    data.records[1]
        .observation
        .territory
        .as_mut()
        .unwrap()
        .neighbors[0]
        .entity_id = Some(EntityId::from_key("earth/missing").unwrap());
    assert!(validate(&catalog(), &data).is_err());
}

#[test]
fn spatial_extent_is_optional_but_cannot_be_an_empty_description() {
    let mut dataset: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/bijie/page-20.dataset.json"
    ))
    .unwrap();
    assert!(dataset.records[1].observation.extent.is_none());
    dataset.records[1].observation.extent = Some("East of the named bridge".into());
    validate(&catalog(), &dataset).unwrap();
    dataset.records[1].observation.extent = Some(" \n\t ".into());
    assert!(
        validate(&catalog(), &dataset)
            .unwrap_err()
            .problem
            .contains("empty spatial extent")
    );
}

#[test]
fn unresolved_coordinate_requires_an_explanation_and_excludes_a_numeric_point() {
    use atlas_geo::coordinates::{Datum, UnresolvedCoordinate};
    let mut dataset: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/bijie/page-20.dataset.json"
    ))
    .unwrap();
    let observation = &mut dataset.records[1].observation;
    let numeric = observation.coordinate.take().unwrap();
    observation.unresolved_coordinate = Some(UnresolvedCoordinate {
        longitude_text: "185°20′".into(),
        latitude_text: "27°17′".into(),
        resolution_arcminutes: 1.0,
        datum: Datum::Unspecified,
        reason: "Printed longitude outside the angular domain.".into(),
    });
    validate(&catalog(), &dataset).unwrap();
    dataset.records[1].observation.coordinate = Some(numeric);
    assert!(
        validate(&catalog(), &dataset)
            .unwrap_err()
            .problem
            .contains("both")
    );
    dataset.records[1].observation.coordinate = None;
    for field in ["reason", "longitude_text", "latitude_text"] {
        let mut raw = serde_json::to_value(&dataset).unwrap();
        raw["records"][1]["observation"]["unresolved_coordinate"][field] = "".into();
        assert!(validate(&catalog(), &serde_json::from_value(raw).unwrap()).is_err());
    }
    for (field, value) in [
        ("resolution_arcminutes", serde_json::json!(0)),
        ("datum", serde_json::json!("moon_iau")),
    ] {
        let mut raw = serde_json::to_value(&dataset).unwrap();
        raw["records"][1]["observation"]["unresolved_coordinate"][field] = value;
        assert!(validate(&catalog(), &serde_json::from_value(raw).unwrap()).is_err());
    }
}

#[test]
fn reviewed_gazetteer_rows_pass_with_original_precision() {
    let dataset: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/bijie/page-20.dataset.json"
    ))
    .unwrap();
    let catalog = catalog();
    validate(&catalog, &dataset).unwrap();
    assert_eq!(dataset.records.len(), 36);
    let commune = dataset
        .records
        .iter()
        .find(|r| r.observation.name == "翠屏公社")
        .unwrap();
    assert_eq!(commune.observation.population, Some(15515));
    assert_eq!(
        commune.observation.coordinate.as_ref().unwrap().datum,
        atlas_geo::coordinates::Datum::Unspecified
    );
}

#[test]
fn invalid_reference_page_identity_and_coordinate_are_rejected() {
    let dataset: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/bijie/page-20.dataset.json"
    ))
    .unwrap();
    let catalog = catalog();
    let mut bad = dataset.clone();
    bad.records[0].scan_page = Some(661);
    assert!(
        validate(&catalog, &bad)
            .unwrap_err()
            .problem
            .contains("page")
    );
    let mut bad = dataset.clone();
    bad.records[0].source_id = "missing".into();
    assert!(
        validate(&catalog, &bad)
            .unwrap_err()
            .problem
            .contains("source")
    );
    let mut bad = dataset.clone();
    bad.records[0].id = bad.records[1].id;
    assert!(
        validate(&catalog, &bad)
            .unwrap_err()
            .problem
            .contains("identity")
    );
    let mut bad = dataset;
    bad.records[1]
        .observation
        .coordinate
        .as_mut()
        .unwrap()
        .point
        .latitude = 91.0;
    assert!(
        validate(&catalog, &bad)
            .unwrap_err()
            .problem
            .contains("coordinate")
    );
}

#[test]
fn former_names_have_no_invented_dates() {
    let dataset: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/liuzhi/page-15.dataset.json"
    ))
    .unwrap();
    let former: Vec<_> = dataset
        .records
        .iter()
        .flat_map(|r| &r.observation.former_names)
        .collect();
    assert_eq!(former.len(), 4);
    assert!(former.iter().all(|name| name.validity.is_none()));
    assert!(former.iter().any(|name| name.name == "灭资大队"));
}

#[test]
fn estimates_require_checked_evidence_and_a_matching_datum() {
    use crate::model::{Estimate, EstimateValue};
    use atlas_geo::coordinates::{Datum, LonLat};
    let mut dataset: Dataset = serde_json::from_str(include_str!(
        "../../../../data/guizhou-1960s/bijie/page-20.dataset.json"
    ))
    .unwrap();
    let subject = &dataset.records[1];
    let key = "editorial/cuiping-location";
    dataset.estimates.push(Estimate {
        id: FactId::from_key(key).unwrap(),
        key: key.into(),
        entity_id: subject.entity_id,
        basis: vec![subject.id],
        brief: "Coordinate interpretation fixture".into(),
        value: EstimateValue::Location {
            point: LonLat {
                longitude: 105.26667,
                latitude: 27.3,
            },
            datum: Datum::Wgs84,
            uncertainty_m: 2000.0,
        },
    });
    validate(&catalog(), &dataset).unwrap();
    dataset.records[1].checked = false;
    assert!(
        validate(&catalog(), &dataset)
            .unwrap_err()
            .problem
            .contains("checked")
    );
    dataset.records[1].checked = true;
    if let EstimateValue::Location { datum, .. } = &mut dataset.estimates[0].value {
        *datum = Datum::MoonIau;
    }
    let error = validate(&catalog(), &dataset).unwrap_err();
    assert!(error.problem.contains("datum"));
    assert!(std::error::Error::source(&error).is_some());
}

#[cfg(test)]
pub(super) fn catalog() -> SourceCatalog {
    SourceCatalog {
        sources: vec![Source {
            id: "bijie-gazetteer".into(),
            title: "贵州省毕节县地名录".into(),
            responsibility: None,
            publication_year: None,
            reference_year: None,
            scan_pages: Some(660),
            scan_sha256: Some(
                "3591c54d1e4438ce87241df13858159c07135a4fd1efffe87df4aa6e1fd91bb1".into(),
            ),
            url: None,
        }],
    }
}
