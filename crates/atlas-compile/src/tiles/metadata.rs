use std::collections::BTreeMap;

use geozero::mvt::TileValue;
use serde_json::json;

use crate::export::{Artifacts, insert};

use super::{TileError, features::ObservationPoint};

pub(super) fn publication_base(value: &str) -> Result<String, TileError> {
    let mut url = url::Url::parse(value)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(TileError::PublicationBase);
    }
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url.to_string())
}

pub(super) fn insert_body(
    output: &mut Artifacts,
    base: &str,
    body: &str,
    maxzoom: u8,
    points: &[ObservationPoint<'_>],
) -> Result<(), TileError> {
    let fields: BTreeMap<_, _> = points
        .iter()
        .flat_map(|point| &point.properties)
        .map(|(key, value)| {
            let kind = match value {
                TileValue::Str(_) => "String",
                TileValue::Bool(_) => "Boolean",
                _ => "Number",
            };
            (key, kind)
        })
        .collect();
    let document = json!({
        "tilejson": "3.0.0",
        "name": format!("Historical Atlas — {body} source observations"),
        "scheme": "xyz",
        "tiles": [format!("{base}tiles/{body}/observations/{{z}}/{{x}}/{{y}}.mvt")],
        "minzoom": 0,
        "maxzoom": maxzoom,
        "bounds": bounds(points),
        "attribution": "Historical Atlas and cited sources · CC BY-SA 4.0",
        "vector_layers": [{
            "id": "observations",
            "description": "Reviewed source coordinates with original notation, datum, and reporting precision.",
            "fields": fields,
            "minzoom": 0,
            "maxzoom": maxzoom
        }],
        "atlas:body": body,
        "atlas:projection": "spherical_mercator",
        "atlas:availability": format!("{base}tiles/{body}/observations/index.json"),
        "atlas:evidence": format!("{base}entities/shards.json"),
        "atlas:sources": format!("{base}sources.json")
    });
    insert(
        output,
        &format!("tiles/{body}/observations.json"),
        &document,
    )?;
    insert(output, &format!("styles/{body}.json"), &style(base, body))?;
    Ok(())
}

fn bounds(points: &[ObservationPoint<'_>]) -> [f64; 4] {
    points
        .iter()
        .fold([180.0, 90.0, -180.0, -90.0], |[w, s, e, n], point| {
            let position = point.coordinate.point;
            let wraps = point.projected.0 < 1.0 / 64.0 || point.projected.0 > 63.0 / 64.0;
            [
                if wraps {
                    -180.0
                } else {
                    w.min(position.longitude)
                },
                s.min(position.latitude),
                if wraps {
                    180.0
                } else {
                    e.max(position.longitude)
                },
                n.max(position.latitude),
            ]
        })
}

fn style(base: &str, body: &str) -> serde_json::Value {
    json!({
        "version": 8,
        "name": format!("Historical Atlas — {body}"),
        "metadata": {"atlas:body": body},
        "sources": {
            "observations": {"type": "vector", "url": format!("{base}tiles/{body}/observations.json")}
        },
        "layers": [
            {"id": "background", "type": "background", "paint": {"background-color": "#f1eee5"}},
            {
                "id": "source-observations", "type": "circle", "source": "observations",
                "source-layer": "observations",
                "paint": {
                    "circle-radius": 4,
                    "circle-color": ["match", ["get", "entity_kind"],
                        "commune", "#a45131", "brigade", "#b88145",
                        "settlement", "#356c68", "#4c5968"],
                    "circle-stroke-color": "#ffffff", "circle-stroke-width": 1,
                    "circle-opacity": 0.85
                }
            }
        ]
    })
}
