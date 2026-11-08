use super::*;

#[test]
fn compiled_ertang_sheet_draws_places_and_keeps_shared_seat_identities() {
    let atlas = crate::catalog::tests::atlas();
    let commune = EntityId::from_key("earth/cn-gz/zhijin/ertang-commune").unwrap();
    let drawings = atlas.drawings(commune);
    assert_eq!(drawings.len(), 1);
    let drawing = drawings[0];
    assert_eq!(drawing.units.len(), 1);
    assert_eq!(drawing.places.len(), 43);
    let symbols = points(&atlas, drawing);
    assert_eq!(symbols.len(), 39);
    let hits = pick(&symbols, |p| {
        ((p.x - 615.0).powi(2) + (p.y - 801.0).powi(2)).sqrt() as f32
    });
    let village = EntityId::from_key("earth/cn-gz/zhijin/ertang/ertang-village").unwrap();
    let brigade = EntityId::from_key("earth/cn-gz/zhijin/ertang/ertang-brigade").unwrap();
    assert_eq!(hits, vec![village, commune, brigade]);
    assert!(pick(&symbols, |_| 9.0).is_empty());
    assert_eq!(atlas.drawings(village)[0].scan_page, 40);
    let table_only = EntityId::from_key("earth/cn-gz/zhijin/ertang/luodi-chong").unwrap();
    assert!(atlas.drawings(table_only).is_empty());
    assert_eq!(
        atlas.search("小河边", atlas_geo::body::BodyId::Earth),
        atlas.search("小河坝", atlas_geo::body::BodyId::Earth)
    );
}

#[test]
fn published_drawings_reject_foreign_records_and_outside_pixels() {
    for outside in [false, true] {
        let mut publication = crate::catalog::tests::publication();
        let commune = EntityId::from_key("earth/cn-gz/zhijin/ertang-commune").unwrap();
        let entry = publication.entries.get_mut(&commune).unwrap();
        let drawing = &mut entry.drawings[0];
        if outside {
            drawing.places[0].point.x = 1047.0;
        } else {
            drawing.places[0].record_id = atlas_core::identity::FactId::from_key(
                "zhijin-gazetteer-scan-b/p33/ertang/overview",
            )
            .unwrap();
        }
        assert!(matches!(
            Atlas::new(publication),
            Err(crate::error::AppError::Sheet(_))
        ));
    }
}

#[test]
fn qimo_drawing_preserves_seat_disagreement_and_separate_commune_symbols() {
    let atlas = crate::catalog::tests::atlas();
    let id = |key| EntityId::from_key(&format!("earth/cn-gz/zhijin/{key}")).unwrap();
    let commune = id("qimo-commune");
    let drawing = atlas.drawings(commune)[0];
    assert_eq!(drawing.scan_page, 36);
    assert_eq!(drawing.places.len(), 57);
    let symbols = points(&atlas, drawing);
    assert_eq!(symbols.len(), 51);
    let hits = pick(&symbols, |p| {
        ((p.x - 279.0).powi(2) + (p.y - 783.0).powi(2)).sqrt() as f32
    });
    assert_eq!(
        hits.into_iter().collect::<std::collections::BTreeSet<_>>(),
        [commune, id("qimo/chenjia-zhai")].into()
    );
    assert!(
        atlas
            .drawings(id("ertang-commune"))
            .iter()
            .all(|sheet| sheet.scan_page == 40)
    );
    let brigade = atlas.entry(id("qimo/mofeng-brigade")).unwrap();
    let seats: std::collections::BTreeSet<_> = brigade
        .records
        .iter()
        .filter_map(|r| r.observation.seat_id)
        .collect();
    assert_eq!(
        seats,
        [id("qimo/jibu"), id("qimo/longtan-riverbank-tian")].into()
    );
    assert!(atlas.drawings(id("qimo/longtan-riverbank-tian")).is_empty());
    assert!(
        atlas
            .entry(id("qimo/jibu"))
            .unwrap()
            .records
            .iter()
            .all(|r| r.observation.coordinate.is_none())
    );
}

#[test]
fn pingqiao_drawing_selects_the_village_brigade_and_commune_at_the_shared_seat() {
    let atlas = crate::catalog::tests::atlas();
    let id = |key| EntityId::from_key(&format!("earth/cn-gz/zhijin/{key}")).unwrap();
    let commune = id("pingqiao-commune");
    let drawing = atlas.drawings(commune)[0];
    assert_eq!(drawing.scan_page, 33);
    assert_eq!(drawing.units.len(), 1);
    assert_eq!(drawing.places.len(), 36);
    assert_eq!(drawing.vertices.len(), 108);
    let symbols = points(&atlas, drawing);
    assert_eq!(symbols.len(), 31);
    let hits = pick(&symbols, |p| {
        ((p.x - 540.0).powi(2) + (p.y - 802.0).powi(2)).sqrt() as f32
    });
    assert_eq!(
        hits.into_iter().collect::<std::collections::BTreeSet<_>>(),
        [
            commune,
            id("pingqiao/laowu-ji"),
            id("pingqiao/pingqiao-brigade")
        ]
        .into()
    );
    assert!(atlas.drawings(id("pingqiao/kengkeng-tian")).is_empty());
    assert_eq!(atlas.drawings(id("pingqiao/luokeng-tian"))[0].scan_page, 33);
    assert_eq!(
        atlas.search("平桥公社", atlas_geo::body::BodyId::Earth),
        vec![commune]
    );
}
