use super::*;
use atlas_ledger::model::EntityKind;

#[test]
fn baiyan_table_keeps_all_settlements_seats_and_separate_area_names() {
    let input =
        crate::input::load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap();
    let output = compile(&input).unwrap();
    let rows: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|r| r.scan_page == Some(501))
        .collect();
    assert_eq!(rows.len(), 25);
    let counts = |kind| {
        rows.iter()
            .filter(|r| {
                input
                    .data
                    .entities
                    .iter()
                    .any(|e| e.id == r.entity_id && e.kind == kind)
            })
            .count()
    };
    assert_eq!(counts(EntityKind::Settlement), 20);
    assert_eq!(counts(EntityKind::Brigade), 2);
    assert_eq!(counts(EntityKind::Toponym), 2);
    for row in rows {
        let published = super::tests::published_record(&output, row);
        assert_eq!(published["scan_page"], 501);
        assert_eq!(published["page_or_sheet"], "466");
        assert_eq!(
            published["observation"]["coordinate"]["resolution_arcminutes"],
            1.0
        );
        assert!(published["source_date"].is_null());
    }
    for (key, population, parent) in [
        ("baiyan/baiyanjiao-brigade", Some(1541), "baiyan-commune"),
        ("baiyan/chuanshandong-brigade", Some(1163), "baiyan-commune"),
        ("baiyan/huomagou", None, "baiyan-commune"),
        ("baiyan/hemagou", None, "baiyan-commune"),
        ("baiyan/zhoujia", Some(71), "baiyan-commune"),
        ("baiyan/shabaobao", Some(110), "baiyan-commune"),
    ] {
        let record = input
            .data
            .records
            .iter()
            .find(|r| r.key == format!("bijie-gazetteer/p466/{key}"))
            .unwrap();
        assert_eq!(record.observation.population, population);
        assert_eq!(
            record.observation.parent_id,
            Some(EntityId::from_key(&format!("earth/cn-gz/bijie/{parent}")).unwrap())
        );
    }
    let commune = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/page-466/baiyan-commune")
        .unwrap();
    assert_eq!(commune.observation.population, Some(2784));
    assert_eq!(
        commune.observation.seat_id,
        Some(EntityId::from_key("earth/cn-gz/bijie/baiyan/baiyanjiao").unwrap())
    );
    assert_eq!(
        input
            .data
            .records
            .iter()
            .find(|r| r.key == "bijie-gazetteer/p466/baiyan/jiadanwan")
            .unwrap()
            .observation
            .name,
        "柳担湾"
    );
}

#[test]
fn baiyan_overview_keeps_the_abbreviated_year_and_its_interpretation_basis() {
    let input =
        crate::input::load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap();
    let record = input
        .data
        .records
        .iter()
        .find(|r| r.key == "bijie-gazetteer/p465/baiyan-commune/establishment")
        .unwrap();
    let date = record.source_date.as_ref().unwrap();
    assert_eq!(date.text, "58年");
    assert!(date.conversion_basis.as_ref().unwrap().contains("1958"));
    assert_eq!(record.page_or_sheet.as_deref(), Some("465"));
    assert_eq!(record.scan_page, Some(500));
}
