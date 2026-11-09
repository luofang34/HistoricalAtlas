use super::*;
use crate::input::{LedgerInput, load_blocking};
use atlas_ledger::model::EntityKind;

#[cfg(test)]
fn input() -> LedgerInput {
    load_blocking(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

#[cfg(test)]
fn id(key: &str) -> EntityId {
    EntityId::from_key(&format!("earth/cn-gz/bijie/urban/{key}")).unwrap()
}

#[test]
fn complete_urban_street_table_preserves_extent_without_creating_point_coordinates() {
    let input = input();
    let output = compile_with_derived(&input, &[]).unwrap();
    let streets: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|record| {
            record.source_id == "bijie-gazetteer"
                && matches!(record.scan_page, Some(50 | 51))
                && record.observation.extent.is_some()
        })
        .collect();
    assert_eq!(streets.len(), 56);
    for record in streets {
        assert!(record.checked);
        assert!(record.observation.coordinate.is_none());
        assert!(record.observation.population.is_none());
        assert!(record.observation.households.is_none());
        let path = PathBuf::from(format!("entities/index/{}.json", record.entity_id.shard()));
        let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
        let published = shard[record.entity_id.0.to_string()]["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["id"] == record.id.0.to_string())
            .unwrap();
        assert_eq!(
            published["observation"]["extent"],
            record.observation.extent.as_deref().unwrap()
        );
        assert!(published["observation"]["coordinate"].is_null());
    }
    assert_eq!(
        input
            .data
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::StreetOffice)
            .count(),
        4
    );
}

#[test]
fn office_sections_continue_across_pages_and_end_at_the_separate_street_heading() {
    let input = input();
    for (office, count) in [
        ("jiefang-road-office", 12),
        ("shuangjing-road-office", 12),
        ("weining-road-office", 12),
        ("qingbi-road-office", 9),
    ] {
        let children: Vec<_> = input
            .data
            .records
            .iter()
            .filter(|record| record.observation.parent_id == Some(id(office)))
            .collect();
        assert_eq!(children.len(), count, "{office}");
    }
    let guihua = input
        .data
        .records
        .iter()
        .find(|record| record.entity_id == id("guihua-road"))
        .unwrap();
    assert_eq!(guihua.scan_page, Some(51));
    assert_eq!(
        guihua.observation.parent_id,
        Some(id("weining-road-office"))
    );
    let town = EntityId::from_key("earth/cn-gz/bijie/bijie-town").unwrap();
    let unassigned: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|record| {
            record.scan_page == Some(51)
                && record.observation.extent.is_some()
                && record.observation.parent_id == Some(town)
        })
        .collect();
    assert_eq!(unassigned.len(), 11);
    assert!(
        unassigned
            .iter()
            .any(|record| record.observation.name == "望成路")
    );
}

#[test]
fn historical_street_names_keep_qualifiers_and_merger_notes_without_invented_dates() {
    let input = input();
    let former: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|record| matches!(record.scan_page, Some(50 | 51)))
        .flat_map(|record| &record.observation.former_names)
        .collect();
    assert_eq!(former.len(), 17);
    assert!(former.iter().all(|name| name.validity.is_none()));
    let old = former.iter().find(|name| name.name == "县署街").unwrap();
    assert_eq!(old.source_date.as_ref().unwrap().text, "清");
    assert!(
        former
            .iter()
            .find(|name| name.name == "半边街")
            .unwrap()
            .source_date
            .is_none()
    );
    let merged = input
        .data
        .records
        .iter()
        .find(|record| record.entity_id == id("yongjun-road"))
        .unwrap();
    assert_eq!(
        merged.observation.note.as_deref(),
        Some("洪山路、敦家湾路合并")
    );
    assert!(merged.observation.former_names.is_empty());
    let ambiguous = input
        .data
        .records
        .iter()
        .find(|record| record.entity_id == id("xinsheng-road"))
        .unwrap();
    assert!(ambiguous.observation.pinyin.is_none());
    assert!(ambiguous.locator.contains("partly illegible"));
}

#[test]
fn homonymous_temple_rows_remain_distinct_at_identical_source_coordinates() {
    let input = input();
    let temples: Vec<_> = input
        .data
        .records
        .iter()
        .filter(|record| record.observation.name == "财神庙")
        .collect();
    assert_eq!(temples.len(), 2);
    assert_ne!(temples[0].entity_id, temples[1].entity_id);
    assert_eq!(
        temples[0].observation.coordinate,
        temples[1].observation.coordinate
    );
    let output = compile_with_derived(&input, &[]).unwrap();
    for record in temples {
        let path = PathBuf::from(format!("entities/index/{}.json", record.entity_id.shard()));
        let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
        assert_eq!(
            shard[record.entity_id.0.to_string()]["records"][0]["observation"]["note"],
            record.observation.note.as_deref().unwrap()
        );
    }
    assert_eq!(
        input
            .data
            .records
            .iter()
            .filter(|record| record.source_id == "bijie-gazetteer"
                && matches!(record.scan_page, Some(52 | 53)))
            .count(),
        23
    );
}

#[test]
fn source_spelling_coordinates_and_addresses_survive_compilation() {
    let input = input();
    let source = input
        .data
        .records
        .iter()
        .find(|record| record.entity_id == id("caishen-lou"))
        .unwrap();
    assert_eq!(source.observation.pinyin.as_deref(), Some("Cháishén Nóu"));
    let output = compile_with_derived(&input, &[]).unwrap();
    let subject = id("anti-japanese-army-headquarters-site");
    let path = PathBuf::from(format!("entities/index/{}.json", subject.shard()));
    let shard: serde_json::Value = serde_json::from_slice(&output[&path]).unwrap();
    let records = shard[subject.0.to_string()]["records"].as_array().unwrap();
    let table = records
        .iter()
        .find(|record| record["scan_page"] == 53)
        .unwrap();
    assert_eq!(
        table["observation"]["coordinate"]["latitude_text"],
        "27°10′"
    );
    assert_eq!(table["observation"]["coordinate"]["datum"], "unspecified");
    let address = records
        .iter()
        .find(|record| record["scan_page"] == 49)
        .unwrap();
    assert_eq!(
        address["observation"]["extent"],
        "周素园先生住宅（今和平路74号）"
    );
    assert!(address["observation"]["coordinate"].is_null());
}
