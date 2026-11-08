use super::*;
use crate::input::load_blocking;
use atlas_core::assertion::AdministrativeRank;
use atlas_geo::coordinates::Datum;

#[test]
fn every_indexed_bijie_commune_publishes_its_own_coordinate_evidence() {
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let output = compile(&input).unwrap();
    let communes: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|r| {
            r.key.starts_with("bijie-gazetteer/contents-")
                && r.observation.rank == Some(AdministrativeRank::Commune)
        })
        .collect();
    assert_eq!(communes.len(), 124);
    let mut parents = BTreeSet::new();
    for commune in communes {
        parents.insert(commune.observation.parent_id.unwrap());
        let path = PathBuf::from(format!("entities/index/{}.json", commune.entity_id.shard()));
        let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
        let published = &shard[commune.entity_id.0.to_string()];
        let records: Vec<Record> = serde_json::from_value(published["records"].clone()).unwrap();
        assert!(records.iter().any(|r| r.id == commune.id));
        let coordinates: Vec<_> = records
            .iter()
            .filter(|r| r.observation.coordinate.is_some())
            .collect();
        assert!(!coordinates.is_empty(), "{}", commune.key);
        for record in coordinates {
            let coordinate = record.observation.coordinate.as_ref().unwrap();
            assert_eq!(record.entity_id, commune.entity_id);
            assert_eq!(record.source_id, "bijie-gazetteer");
            assert_eq!(coordinate.datum, Datum::Unspecified);
            assert_eq!(coordinate.resolution_arcminutes, 1.0);
            let page: u32 = record.page_or_sheet.as_ref().unwrap().parse().unwrap();
            assert_eq!(record.scan_page, Some(page + 35));
        }
    }
    assert_eq!(parents.len(), 15);
}

#[test]
fn commune_rows_keep_their_printed_values_when_nearby_rows_differ() {
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let output = compile(&input).unwrap();
    for (page, slug, longitude, latitude) in [
        (242, "ledu", "105°40′", "27°38′"),
        (278, "daji", "105°32′", "27°32′"),
        (301, "qingshuipu", "105°31′", "27°40′"),
        (354, "shakua", "105°14′", "27°10′"),
        (382, "xiamuying", "104°59′", "27°08′"),
        (436, "changba", "105°20′", "27°35′"),
    ] {
        let key = format!("bijie-gazetteer/page-{page}/{slug}-commune");
        let record = input.data.records.iter().find(|r| r.key == key).unwrap();
        let published = super::tests::published_record(&output, record);
        let coordinate = &published["observation"]["coordinate"];
        assert_eq!(coordinate["longitude_text"], longitude);
        assert_eq!(coordinate["latitude_text"], latitude);
        assert_eq!(published["page_or_sheet"], page.to_string());
        assert_eq!(published["scan_page"], page + 35);
        assert!(published["source_date"].is_null());
    }
}

#[test]
fn anomalous_commune_longitudes_publish_alongside_independent_map_locations() {
    let input = load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let output = compile(&input).unwrap();
    for (slug, page, longitude) in [("hantun", 186, "107°27′"), ("haogou", 188, "105°20′")] {
        assert_independent_map_location(&output, slug, page, longitude);
    }
}

#[cfg(test)]
fn assert_independent_map_location(output: &Artifacts, slug: &str, page: u32, longitude: &str) {
    let id = EntityId::from_key(&format!("earth/cn-gz/bijie/{slug}-commune")).unwrap();
    let path = PathBuf::from(format!("entities/index/{}.json", id.shard()));
    let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
    let published = &shard[id.0.to_string()];
    let records: Vec<Record> = serde_json::from_value(published["records"].clone()).unwrap();
    let table = records
        .iter()
        .find(|r| r.key == format!("bijie-gazetteer/p{page}/{slug}-commune"))
        .unwrap();
    let coordinate = table.observation.coordinate.as_ref().unwrap();
    assert_eq!(coordinate.longitude_text, longitude);
    let map_record = records
        .iter()
        .find(|r| r.key == format!("bijie-gazetteer/county-map/{slug}-commune"))
        .unwrap();
    assert_eq!(map_record.scan_page, Some(658));
    assert!(map_record.observation.coordinate.is_none());
    let estimates: Vec<Estimate> = serde_json::from_value(published["estimates"].clone()).unwrap();
    let estimate = estimates
        .iter()
        .find(|e| {
            matches!(e.value, EstimateValue::MapLocation { record_id, .. }
            if record_id == map_record.id)
        })
        .unwrap();
    let EstimateValue::MapLocation {
        point,
        datum,
        uncertainty_m,
        ..
    } = estimate.value
    else {
        panic!("registered location expected")
    };
    assert!((105.4..105.5).contains(&point.longitude));
    assert!((27.3..27.4).contains(&point.latitude));
    assert_eq!(datum, Datum::Unspecified);
    assert_eq!(uncertainty_m, 3500.0);
    assert!(!estimate.basis.contains(&table.id));
}
