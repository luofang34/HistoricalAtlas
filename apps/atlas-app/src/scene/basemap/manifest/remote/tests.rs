use super::super::Manifest;
use crate::scene::DisplayProjection;
use atlas_geo::body::BodyId;
use serde_json::{Value, json};

#[cfg(test)]
fn layer_ids(style: &Value) -> Vec<&str> {
    style["layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|layer| layer["id"].as_str().unwrap())
        .collect()
}

#[cfg(test)]
fn style(body: BodyId) -> (Value, maplibre::style::Style) {
    let points = json!({"type": "FeatureCollection", "features": []});
    let style = crate::scene::style_with_basemap(
        points,
        body,
        DisplayProjection::Globe,
        &Manifest::bundled().unwrap(),
    )
    .unwrap();
    (serde_json::to_value(&style).unwrap(), style)
}

#[test]
fn earth_draws_sentinel_imagery_over_natural_earth_with_mapterhorn_terrain() {
    let (value, style) = style(BodyId::Earth);
    let ids = layer_ids(&value);
    let position = |id: &str| ids.iter().position(|candidate| *candidate == id).unwrap();
    assert!(position("basemap-natural-earth-0") < position("sentinel-2-cloudless-2016"));
    assert!(position("sentinel-2-cloudless-2016") < position("graticule"));
    assert!(position("mapterhorn-terrain-hillshade") < position("graticule"));
    let terrain = style.terrain.unwrap();
    assert_eq!(terrain.source, "mapterhorn-terrain");
    assert_eq!(value["sources"]["mapterhorn-terrain"]["type"], "raster-dem");
    assert_eq!(
        value["sources"]["mapterhorn-terrain"]["encoding"],
        "terrarium"
    );
    let manifest = Manifest::bundled().unwrap();
    let credits = manifest.attribution(BodyId::Earth);
    assert!(credits.contains("EOX") && credits.contains("Mapterhorn"));
}

#[test]
fn other_bodies_hide_earth_imagery_and_get_no_earth_terrain() {
    let (value, style) = style(BodyId::Moon);
    assert!(style.terrain.is_none());
    let sentinel = value["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|layer| layer["id"] == "sentinel-2-cloudless-2016")
        .unwrap();
    assert_eq!(sentinel["layout"]["visibility"], "none");
    let manifest = Manifest::bundled().unwrap();
    assert!(manifest.terrain(BodyId::Moon).is_none());
    assert!(!manifest.attribution(BodyId::Moon).contains("EOX"));
    assert!(
        manifest
            .layers()
            .iter()
            .any(|(id, body)| id == "mapterhorn-terrain-hillshade" && *body == BodyId::Earth)
    );
}

#[test]
fn services_need_https_templates_licences_and_distinct_names() {
    let base = || {
        let mut manifest: Value = serde_json::from_slice(include_bytes!(
            "../../../../../assets/basemap/manifest.json"
        ))
        .unwrap();
        manifest["remote"] = json!([{"id": "imagery", "type": "raster",
            "tiles": "https://example.org/{z}/{x}/{y}.jpg", "tile_size": 256, "maxzoom": 10,
            "attribution": "Example", "license": "CC0"}]);
        manifest
    };
    let parse = |manifest: &Value| Manifest::parse(&serde_json::to_vec(manifest).unwrap());
    assert!(parse(&base()).is_ok());
    for (field, value) in [
        ("tiles", json!("http://example.org/{z}/{x}/{y}.jpg")),
        ("tiles", json!("https://example.org/{z}/{x}.jpg")),
        ("license", json!(" ")),
        ("tile_size", json!(300)),
        ("id", json!("Imagery")),
    ] {
        let mut manifest = base();
        manifest["remote"][0][field] = value;
        assert!(parse(&manifest).is_err(), "{field}");
    }
    let mut duplicate = base();
    let copy = duplicate["remote"][0].clone();
    duplicate["remote"].as_array_mut().unwrap().push(copy);
    assert!(parse(&duplicate).is_err());
    let mut terrain = base();
    terrain["remote"][0]["type"] = json!("terrain");
    terrain["remote"][0]["encoding"] = json!("png");
    terrain["remote"][0]["exaggeration"] = json!(1.0);
    assert!(parse(&terrain).is_err());
}
