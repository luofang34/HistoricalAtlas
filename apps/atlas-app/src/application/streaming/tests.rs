use super::*;
use atlas_boundary::publication::{ShardPlateRef, ShardTemporalSummary, VerifiedPart};
use atlas_geo::{
    body::BodyId,
    plates::{PlateId, RotationSample, RotationSequence},
    rotation::EulerRotation,
};
use atlas_ledger::plates::PlateModelDocument;

fn test_models() -> (
    BTreeMap<String, PlateModelDocument>,
    BTreeMap<String, RotationModel>,
) {
    let sequences = vec![RotationSequence {
        moving_plate: PlateId(602),
        fixed_plate: PlateId(0),
        samples: vec![
            RotationSample {
                age_ma: 0.0,
                rotation: EulerRotation::IDENTITY,
                source_line: 1,
                description: String::new(),
            },
            RotationSample {
                age_ma: 200.0,
                rotation: EulerRotation {
                    pole: LonLat {
                        longitude: 0.0,
                        latitude: 90.0,
                    },
                    angle_degrees: -40.0,
                },
                source_line: 2,
                description: String::new(),
            },
        ],
    }];
    let doc = PlateModelDocument {
        id: "test-model".into(),
        source_id: "src".into(),
        source_sha256: "0".repeat(64),
        attribution: "Test".into(),
        license: "CC0-1.0".into(),
        reference_frame: "paleomagnetic".into(),
        present_epoch: EpochTime::from_ticks(0),
        youngest_ma: 0.0,
        oldest_ma: 200.0,
        sequences: sequences.clone(),
    };
    let model = RotationModel::from_sequences(sequences).unwrap();
    (
        BTreeMap::from([("test-model".to_owned(), doc)]),
        BTreeMap::from([("test-model".to_owned(), model)]),
    )
}

#[test]
fn rotated_shard_envelope_matches_paleogeographic_viewport_and_prunes_undated_shards() {
    let (plate_models, rotation_models) = test_models();
    let part = VerifiedPart {
        path: "entities/index/earth/detail/4_12_5.json".into(),
        bytes: 128,
        sha256: "0".repeat(64),
    };
    let undated = ShardSummary {
        id: "earth/detail/4_12_5/0".into(),
        body: BodyId::Earth,
        overview: false,
        part: part.clone(),
        entity_count: 10,
        bounds: Some([100.0, 20.0, 105.0, 25.0]),
        temporal: ShardTemporalSummary::default(),
        depends_on: Vec::new(),
    };
    let tectonic = ShardSummary {
        id: "earth/detail/4_12_5/1".into(),
        body: BodyId::Earth,
        overview: false,
        part,
        entity_count: 2,
        bounds: Some([100.0, 20.0, 105.0, 25.0]),
        temporal: ShardTemporalSummary {
            plates: vec![ShardPlateRef {
                model_id: "test-model".into(),
                plate_id: PlateId(602),
            }],
            ..Default::default()
        },
        depends_on: Vec::new(),
    };
    let at_100_ma = EpochTime::years_before(EpochTime::from_ticks(0), 100_000_000).unwrap();
    assert!(!shard_matches_view(
        &undated,
        Some([99.0, 19.0, 106.0, 26.0]),
        MapLayer::SelectedTime,
        at_100_ma,
        &plate_models,
        &rotation_models,
    ));
    assert!(shard_matches_view(
        &undated,
        Some([99.0, 19.0, 106.0, 26.0]),
        MapLayer::MappedPlaces,
        at_100_ma,
        &plate_models,
        &rotation_models,
    ));
    // At 100 Ma, -20° rotation around North Pole shifts [100..105] longitude to [80..85].
    assert!(!shard_matches_view(
        &tectonic,
        Some([99.0, 19.0, 106.0, 26.0]),
        MapLayer::SelectedTime,
        at_100_ma,
        &plate_models,
        &rotation_models,
    ));
    assert!(shard_matches_view(
        &tectonic,
        Some([79.0, 19.0, 86.0, 26.0]),
        MapLayer::SelectedTime,
        at_100_ma,
        &plate_models,
        &rotation_models,
    ));
    // Zoomed-out globe view (bounds = None) streams active plate shards in SelectedTime.
    assert!(shard_matches_view(
        &tectonic,
        None,
        MapLayer::SelectedTime,
        at_100_ma,
        &plate_models,
        &rotation_models,
    ));
}
