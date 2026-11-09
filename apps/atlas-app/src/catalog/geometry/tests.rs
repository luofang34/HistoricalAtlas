use super::*;
use atlas_boundary::publication::{GeometryPart, fingerprint};

#[test]
fn unloaded_geometry_attaches_only_after_evidence_validation() {
    let mut publication = crate::scene::boundaries::tests::lunar_publication();
    let entry = publication.entries.values_mut().next().unwrap();
    let id = entry.entity.id;
    let bundle = GeometryBundle {
        boundaries: std::mem::take(&mut entry.boundaries),
        ..Default::default()
    };
    let bytes = serde_json::to_vec(&bundle).unwrap();
    let reference = GeometryReference {
        parts: vec![GeometryPart {
            path: "geometry/test/000.part".into(),
            bytes: bytes.len(),
            sha256: fingerprint(&bytes),
        }],
        bounds: Some([1., 1., 2., 2.]),
        subjects: vec![id],
    };
    entry.geometry = Some(reference.clone());
    let mut atlas = Atlas::new(publication).unwrap();
    assert!(atlas.entry(id).unwrap().boundaries.is_empty());
    assert_eq!(atlas.geometry_references(BodyId::Moon).count(), 1);
    let mut wrong = reference.decode(std::slice::from_ref(&bytes)).unwrap();
    wrong.boundaries[0].unit.record_id =
        atlas_core::identity::FactId::from_key("missing/record").unwrap();
    assert!(atlas.attach_geometry(id, wrong).is_err());
    assert!(atlas.entry(id).unwrap().boundaries.is_empty());
    atlas
        .attach_geometry(id, reference.decode(&[bytes]).unwrap())
        .unwrap();
    assert_eq!(atlas.entry(id).unwrap().boundaries.len(), 1);
    assert_eq!(atlas.geometry_references(BodyId::Moon).count(), 0);
}
