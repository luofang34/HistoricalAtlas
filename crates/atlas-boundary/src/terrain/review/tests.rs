use super::*;
use atlas_geo::registration::PlanarPoint;

#[cfg(test)]
fn square(x: f64, y: f64) -> MultiPolygon<f64> {
    crate::polygon::resolve(
        &[(x, y), (x + 1., y), (x + 1., y + 1.), (x, y + 1.)]
            .into_iter()
            .map(|(x, y)| PlanarPoint { x, y })
            .collect::<Vec<_>>(),
        &[crate::polygon::IndexedPolygon {
            exterior: vec![0, 1, 2, 3, 0],
            interiors: vec![],
        }],
    )
    .unwrap()
}

#[test]
fn adjacency_requires_a_shared_edge_and_unmapped_neighbors_remain_unknown() {
    let data: Dataset = serde_json::from_str(include_str!(
        "../../../../../data/guizhou-1960s/bijie/baiyan-overview.dataset.json"
    ))
    .unwrap();
    let record = &data.records[0];
    let names = &record.observation.territory.as_ref().unwrap().neighbors;
    let geometry = BTreeMap::from([
        (names[0].entity_id.unwrap(), square(1., 0.)),
        (names[2].entity_id.unwrap(), square(1., 1.)),
        (names[3].entity_id.unwrap(), square(3., 0.)),
    ]);
    let comparison = comparison(record, &square(0., 0.), &geometry).unwrap();
    assert_eq!(
        comparison
            .neighbors
            .iter()
            .map(|n| n.shares_edge)
            .collect::<Vec<_>>(),
        [Some(true), None, Some(false), Some(false)]
    );
    assert!((12_000.0..12_500.0).contains(&comparison.modeled_area_square_km));
    assert_eq!(comparison.reported_area_square_km, 12.5);
}
