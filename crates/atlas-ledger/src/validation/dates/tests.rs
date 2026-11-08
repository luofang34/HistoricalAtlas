use super::*;
use crate::model::{Dataset, FormerName, SourceCatalog};

#[test]
fn era_dates_require_catalogued_calendar_evidence_including_former_names() {
    let mut data: Dataset = serde_json::from_str(include_str!(
        "../../../../../data/guizhou-1960s/bijie/early-history.dataset.json"
    ))
    .unwrap();
    let catalog: SourceCatalog = serde_json::from_value(serde_json::json!({
        "sources": [{"id":"academia-sinica-calendar", "title":"兩千年中西曆轉換",
        "url":"https://sinocal.sinica.edu.tw/"}]
    }))
    .unwrap();
    let sources = super::super::citations::validate_sources(&catalog).unwrap();
    for record in &data.records {
        validate_dates(record, &sources).unwrap();
        if record
            .source_date
            .as_ref()
            .unwrap()
            .era_conversion
            .is_some()
        {
            assert!(
                validate_dates(record, &BTreeMap::new())
                    .unwrap_err()
                    .problem
                    .contains("calendar source")
            );
        }
    }
    let record = &mut data.records[0];
    let mut date = record.source_date.take().unwrap();
    date.era_conversion.as_mut().unwrap().period = atlas_core::calendar::era::EraPeriod::Month {
        month: 0,
        leap: false,
    };
    record.observation.former_names.push(FormerName {
        name: "卢鹿部".into(),
        source_date: Some(date),
        validity: None,
    });
    let error = validate_dates(record, &sources).unwrap_err();
    assert_eq!(error.context, record.key);
    assert!(matches!(
        error.source,
        Some(super::super::error::ValidationCause::Calendar(_))
    ));
}
