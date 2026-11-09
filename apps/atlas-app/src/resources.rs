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
    let names = crate::fonts::names(&bytes("assets/fonts.json").await?)
        .map_err(|error| SourceFetchError(Box::new(error)))?;
    let mut files = Vec::new();
    for name in names {
        let data = bytes(&format!("assets/{name}")).await?;
        files.push((name, data));
    }
    crate::fonts::install(context, files);
    Ok(())
}
