use super::*;
use crate::selection::TimeControl;

#[test]
fn real_wuliping_source_network_has_seat_links_and_time_view_excludes_undated_hierarchy() {
    let atlas = crate::catalog::tests::atlas();
    let id = EntityId::from_key("earth/cn-gz/bijie/wuliping-commune").unwrap();
    let time = SelectedTime::query(&atlas, &TimeControl::default()).unwrap();
    let source = features(&atlas, id, &time, MapLayer::Observations).unwrap();
    assert!(
        source
            .iter()
            .any(|feature| feature["properties"]["relation"] == "seat")
    );
    assert!(
        source
            .iter()
            .any(|feature| feature["geometry"]["type"] == "MultiLineString")
    );
    assert!(
        source
            .iter()
            .all(|feature| feature["properties"]["scope"]["source_id"] == "bijie-gazetteer")
    );
    assert!(
        features(&atlas, id, &time, MapLayer::SelectedTime)
            .unwrap()
            .is_empty()
    );
    let points = serde_json::json!({"type": "FeatureCollection", "features": source});
    let style = crate::scene::style(points).unwrap();
    assert!(
        style
            .layers
            .iter()
            .any(|layer| layer.id == "territory-links")
    );
}
