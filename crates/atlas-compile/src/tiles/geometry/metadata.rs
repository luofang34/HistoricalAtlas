use super::*;

pub(super) fn publish(
    output: &mut Artifacts,
    base: &str,
    body: &str,
    maxzoom: u8,
    features: &[Feature],
) -> Result<(), TileError> {
    let mut layers = BTreeMap::<&str, BTreeMap<&str, &str>>::new();
    for feature in features {
        let fields = layers.entry(feature.layer).or_default();
        for (key, value) in &feature.properties {
            fields.insert(
                key,
                match value {
                    TileValue::Str(_) => "String",
                    TileValue::Bool(_) => "Boolean",
                    _ => "Number",
                },
            );
        }
    }
    let layers: Vec<_> = layers
        .into_iter()
        .map(|(id, fields)| json!({"id":id,"fields":fields,"minzoom":0,"maxzoom":maxzoom}))
        .collect();
    insert(
        output,
        &format!("tiles/{body}/geometry.json"),
        &json!({
        "tilejson":"3.0.0","name":format!("Historical Atlas — {body} registered geometry"),
        "tiles":[format!("{base}tiles/{body}/geometry/{{z}}/{{x}}/{{y}}.mvt")],"scheme":"xyz",
        "minzoom":0,"maxzoom":maxzoom,"vector_layers":layers,
        "atlas:body":body,"atlas:time":"J2000 TT milliseconds as decimal strings; valid_to is exclusive",
        "atlas:availability":format!("{base}tiles/{body}/geometry/index.json"),
        "attribution":"Historical Atlas and cited sources · CC BY-SA 4.0"}),
    )?;
    let path = format!("styles/{body}.json");
    let mut style: Value = match output.get(std::path::Path::new(&path)) {
        Some(bytes) => serde_json::from_slice(bytes)?,
        None => json!({"version":8,"sources":{},"layers":[]}),
    };
    let sources = style
        .get_mut("sources")
        .and_then(Value::as_object_mut)
        .ok_or(TileError::Geometry("style has no sources"))?;
    sources.insert(
        "geometry".into(),
        json!({"type":"vector","url":format!("{base}tiles/{body}/geometry.json")}),
    );
    if let Some(layers) = style.get_mut("layers").and_then(Value::as_array_mut) {
        layers.extend([
            json!({"id":"boundary-fill","source":"geometry","source-layer":"boundaries","type":"fill","paint":{"fill-color":"#356c68","fill-opacity":0.12}}),
            json!({"id":"boundary-outline","source":"geometry","source-layer":"boundaries","type":"line","paint":{"line-color":"#356c68","line-width":1.5}}),
            json!({"id":"registered-lines","source":"geometry","source-layer":"lines","type":"line","paint":{"line-color":"#356c68","line-width":2}}),
            json!({"id":"map-locations","source":"geometry","source-layer":"map_locations","type":"circle","paint":{"circle-color":"#356c68","circle-radius":4}}),
        ]);
    }
    insert(output, &path, &style)?;
    Ok(())
}
