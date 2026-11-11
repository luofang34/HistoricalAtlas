//! Browser resources loaded independently of executable code.

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Browser static asset fetch or decoding failure.
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    /// Requested asset returned HTTP 404.
    #[error("asset not found: {0}")]
    NotFound(String),
    /// Requested asset returned a non-success HTTP status.
    #[error("asset {url}: HTTP {status} {reason}")]
    Http {
        /// Requested URL.
        url: String,
        /// HTTP status code.
        status: u16,
        /// HTTP status text.
        reason: String,
    },
    /// Browser fetch or buffer promise failed.
    #[error("browser resource: {0}")]
    Transport(String),
    /// Font inventory or subset failed validation.
    #[error(transparent)]
    Font(#[from] crate::fonts::FontError),
}

fn transport(error: wasm_bindgen::JsValue) -> ResourceError {
    ResourceError::Transport(format!("{error:?}"))
}

/// Fetch a same-origin static asset, retaining HTTP failure classification.
pub async fn bytes(url: &str) -> Result<Vec<u8>, ResourceError> {
    let window =
        web_sys::window().ok_or_else(|| ResourceError::Transport("window unavailable".into()))?;
    let response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(transport)?;
    let response: web_sys::Response = response.dyn_into().map_err(transport)?;
    if response.status() == 404 {
        return Err(ResourceError::NotFound(url.to_owned()));
    }
    if !response.ok() {
        return Err(ResourceError::Http {
            url: url.to_owned(),
            status: response.status(),
            reason: response.status_text(),
        });
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(transport)?)
        .await
        .map_err(transport)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// Load publication font subsets while retaining egui's Latin and symbol coverage.
pub async fn fonts(context: &egui::Context) -> Result<(), ResourceError> {
    let names = crate::fonts::names(&bytes("assets/fonts.json").await?)?;
    let mut files = Vec::new();
    for name in names {
        let data = bytes(&format!("assets/{name}")).await?;
        files.push((name, data));
    }
    crate::fonts::install(context, files);
    Ok(())
}
