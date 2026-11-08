use std::collections::BTreeMap;

use atlas_core::identity::EntityId;
use atlas_geo::coordinates::LonLat;
use serde_json::{Value, json};

use super::{
    NodalTerritory, NodeAnchor, RelationEvidence, RelationKind, TerritoryLink, TerritoryNode,
};

struct Endpoint<'a> {
    anchor_entity: EntityId,
    anchor: &'a NodeAnchor,
    seat_evidence: Option<&'a RelationEvidence>,
}

impl NodalTerritory {
    /// Draw located nodes and schematic links; all relationships remain in the network document.
    pub fn features(&self) -> Vec<Value> {
        let nodes: BTreeMap<_, _> = self
            .nodes
            .iter()
            .map(|node| (node.entity_id, node))
            .collect();
        let mut features = Vec::new();
        for node in &self.nodes {
            for endpoint in self.endpoints(node.entity_id, &nodes) {
                let point = endpoint.anchor.coordinate.point;
                features.push(json!({"type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [point.longitude, point.latitude]},
                    "properties": {
                        "geometry_role": "territory_node", "geometry_kind": "nodal_territory",
                        "territory_id": self.query.entity_id, "entity_id": node.entity_id,
                        "body": self.body, "scope": self.query.scope,
                        "anchor_entity_id": endpoint.anchor_entity, "anchor": endpoint.anchor,
                        "subject_evidence": node.evidence,
                        "seat_evidence": endpoint.seat_evidence
                    }
                }));
            }
        }
        for link in &self.links {
            for from in self.endpoints(link.from, &nodes) {
                for to in self.endpoints(link.to, &nodes) {
                    if from.anchor.coordinate.datum != to.anchor.coordinate.datum {
                        continue;
                    }
                    let segments =
                        segments(from.anchor.coordinate.point, to.anchor.coordinate.point);
                    if !segments.is_empty() {
                        features.push(self.link_feature(link, &from, &to, segments));
                    }
                }
            }
        }
        features
    }

    /// Assemble the network's map features into one in-memory GeoJSON collection.
    pub fn geojson(&self) -> Value {
        json!({"type": "FeatureCollection", "features": self.features()})
    }

    fn endpoints<'a>(
        &'a self,
        id: EntityId,
        nodes: &BTreeMap<EntityId, &'a TerritoryNode>,
    ) -> Vec<Endpoint<'a>> {
        let Some(node) = nodes.get(&id) else {
            return Vec::new();
        };
        if !node.anchors.is_empty() {
            return node
                .anchors
                .iter()
                .map(|anchor| Endpoint {
                    anchor_entity: id,
                    anchor,
                    seat_evidence: None,
                })
                .collect();
        }
        self.links
            .iter()
            .filter(|link| link.from == id && link.relation == RelationKind::Seat)
            .flat_map(|link| {
                nodes.get(&link.to).into_iter().flat_map(move |seat| {
                    seat.anchors.iter().map(move |anchor| Endpoint {
                        anchor_entity: link.to,
                        anchor,
                        seat_evidence: Some(&link.evidence),
                    })
                })
            })
            .collect()
    }

    fn link_feature(
        &self,
        link: &TerritoryLink,
        from: &Endpoint<'_>,
        to: &Endpoint<'_>,
        segments: Vec<[[f64; 2]; 2]>,
    ) -> Value {
        json!({"type": "Feature",
            "geometry": {"type": "MultiLineString", "coordinates": segments},
            "properties": {
                "geometry_role": "territory_link", "geometry_kind": "nodal_territory",
                "territory_id": self.query.entity_id, "body": self.body, "scope": self.query.scope,
                "from": link.from, "to": link.to, "relation": link.relation, "evidence": link.evidence,
                "from_anchor_entity": from.anchor_entity, "to_anchor_entity": to.anchor_entity,
                "from_anchor": from.anchor, "to_anchor": to.anchor,
                "from_seat_evidence": from.seat_evidence, "to_seat_evidence": to.seat_evidence
            }
        })
    }
}

fn segments(from: LonLat, to: LonLat) -> Vec<[[f64; 2]; 2]> {
    let a = [from.longitude, from.latitude];
    let b = [to.longitude, to.latitude];
    let delta = to.longitude - from.longitude;
    if delta.abs() <= 180.0 {
        return if a == b { Vec::new() } else { vec![[a, b]] };
    }
    let adjusted = if delta > 0.0 {
        to.longitude - 360.0
    } else {
        to.longitude + 360.0
    };
    if adjusted == from.longitude {
        let b = [adjusted, to.latitude];
        return if a == b { Vec::new() } else { vec![[a, b]] };
    }
    let seam = if delta > 0.0 { -180.0 } else { 180.0 };
    let fraction = (seam - from.longitude) / (adjusted - from.longitude);
    let latitude = from.latitude + fraction * (to.latitude - from.latitude);
    [[a, [seam, latitude]], [[-seam, latitude], b]]
        .into_iter()
        .filter(|[start, end]| start != end)
        .collect()
}

#[cfg(test)]
mod tests;
