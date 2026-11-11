use super::*;
use serde_json::json;

#[cfg(test)]
fn line() -> TracedLine {
    TracedLine {
        record_id: FactId::from_key("survey/river").unwrap(),
        paths: vec![vec![0, 1, 2], vec![3, 4]],
        validity: None,
        validity_basis: Vec::new(),
    }
}

#[test]
fn sampling_preserves_open_ends_and_disjoint_components() {
    let vertices =
        [(0., 0.), (10., 0.), (10., 10.), (20., 0.), (30., 0.)].map(|(x, y)| PlanarPoint { x, y });
    let (points, lines) = densify(&vertices, &[line()], 3.).unwrap();
    let paths = resolve(&points, &lines[0].paths).unwrap();
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].first(), Some(&[0., 0.]));
    assert_eq!(paths[0].last(), Some(&[10., 10.]));
    assert_eq!(paths[1].first(), Some(&[20., 0.]));
    for path in paths {
        for pair in path.windows(2) {
            assert!((pair[0][0] - pair[1][0]).hypot(pair[0][1] - pair[1][1]) <= 3.);
        }
    }
    let mut invalid = line();
    invalid.paths[0] = vec![0, 0];
    assert!(validate(&vertices, &invalid).is_err());
    assert!(densify(&vertices, &[line()], 0.).is_err());
    assert!(densify(&vertices, &[line()], 0.0000001).is_err());
}

#[test]
fn geographic_lines_keep_citation_body_and_component_identity() {
    let entity = EntityId::from_key("moon/test/rille").unwrap();
    let record = line().record_id;
    let data: Dataset = serde_json::from_value(json!({
        "entities":[{"id":entity,"key":"moon/test/rille","body":"moon","kind":{"hydrography":"river"}}],
        "records":[{"id":record,"key":"survey/river","entity_id":entity,"source_id":"survey",
        "locator":"Rille A","checked":true,"observation":{"name":"Rille A"}}],
        "estimates":[],"disputes":[]
    }))
    .unwrap();
    let mut geometry = GeographicLine {
        line: line(),
        datum: Datum::MoonIau,
        vertices: [(1., 1.), (2., 1.), (2., 2.), (3., 1.), (4., 1.)]
            .map(|(x, y)| PlanarPoint { x, y })
            .to_vec(),
        registration: "maps/survey.json".into(),
    };
    let feature = geometry.feature(&data, Some(entity)).unwrap();
    assert_eq!(feature["geometry"]["type"], "MultiLineString");
    assert_eq!(
        feature["geometry"]["coordinates"].as_array().unwrap().len(),
        2
    );
    assert_eq!(feature["properties"]["record_id"], json!(record));
    assert_eq!(feature["properties"]["selected"], true);
    geometry.datum = Datum::Wgs84;
    assert!(geometry.feature(&data, None).is_err());
}
