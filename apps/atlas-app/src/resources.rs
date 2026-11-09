//! Browser resources loaded independently of executable code.

use maplibre::io::source_client::SourceFetchError;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

#[derive(Debug, thiserror::Error)]
#[error("browser resource: {0}")]
struct BrowserError(String);

fn transport(error: wasm_bindgen::JsValue) -> SourceFetchError {
    SourceFetchError::temporary(BrowserError(format!("{error:?}")))
}

/// Fetch a same-origin static asset, retaining HTTP failure classification.
pub async fn bytes(url: &str) -> Result<Vec<u8>, SourceFetchError> {
    let window = web_sys::window().ok_or_else(|| transport("window unavailable".into()))?;
    let response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(transport)?;
    let response: web_sys::Response = response.dyn_into().map_err(transport)?;
    if response.status() == 404 {
        return Err(SourceFetchError::not_found(url));
    }
    if !response.ok() {
        return Err(SourceFetchError::http_response(
            url,
            response.status(),
            BrowserError(response.status_text()),
        ));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(transport)?)
        .await
        .map_err(transport)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// Load publication font subsets while retaining egui's Latin and symbol coverage.
pub async fn fonts(context: &egui::Context) -> Result<(), SourceFetchError> {
    let mut fonts = egui::FontDefinitions::default();
    for name in ["AtlasCJK.otf", "AtlasHanSupplement.ttf"] {
        let data = bytes(&format!("assets/{name}")).await?;
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_owned(data).into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.into());
        }
    }
    context.set_fonts(fonts);
    Ok(())
}
