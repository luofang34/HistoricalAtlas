use super::*;

#[test]
fn online_citations_require_a_source_url_and_locator_without_invented_pages() {
    let mut catalog: SourceCatalog = serde_json::from_value(serde_json::json!({
        "sources": [{"id": "nasa-apollo11", "title": "Apollo 11",
        "responsibility": "NASA", "url": "https://nssdc.gsfc.nasa.gov/planetary/lunar/apollo11info.html"}]
    })).unwrap();
    validate_sources(&catalog).unwrap();
    let mut data: crate::model::Dataset = serde_json::from_str(include_str!(
        "../../../../../data/guizhou-1960s/bijie/page-20.dataset.json"
    ))
    .unwrap();
    let record = &mut data.records[0];
    record.page_or_sheet = None;
    record.scan_page = None;
    validate_citation(record, &catalog.sources[0]).unwrap();
    record.scan_page = Some(1);
    assert!(validate_citation(record, &catalog.sources[0]).is_err());
    record.scan_page = None;
    record.locator.clear();
    assert!(validate_citation(record, &catalog.sources[0]).is_err());
    for url in [
        None,
        Some("javascript:alert(1)"),
        Some("https:///missing-host"),
        Some("https://example.com/a b"),
    ] {
        catalog.sources[0].url = url.map(str::to_owned);
        assert!(validate_sources(&catalog).is_err());
    }
}

#[test]
fn scan_metadata_must_remain_complete_even_with_an_online_copy() {
    let mut catalog = crate::validation::tests::catalog();
    catalog.sources[0].url = Some("https://example.com/book.pdf".into());
    validate_sources(&catalog).unwrap();
    catalog.sources[0].scan_pages = None;
    assert!(validate_sources(&catalog).is_err());
    catalog.sources[0].scan_sha256 = None;
    validate_sources(&catalog).unwrap();
    catalog.sources[0].scan_pages = Some(660);
    assert!(validate_sources(&catalog).is_err());
}
