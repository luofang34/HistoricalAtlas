//! Browser startup from the compiler's publication shards.

use wasm_bindgen::prelude::*;

/// Validate the loaded publication and start the common atlas application.
#[wasm_bindgen]
pub fn start(
    canvas: web_sys::HtmlCanvasElement,
    publication: &str,
) -> Result<BrowserAtlas, JsValue> {
    let atlas = serde_json::from_str(publication)
        .map_err(|error| JsValue::from_str(&format!("publication JSON: {error}")))?;
    let atlas =
        crate::catalog::Atlas::new(atlas).map_err(|error| JsValue::from_str(&error.to_string()))?;
    crate::host::spawn(atlas, canvas)
        .map(BrowserAtlas)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

/// Handle for browser composition, paste, and editing keys.
#[wasm_bindgen]
pub struct BrowserAtlas(crate::host::BrowserControl);

#[wasm_bindgen]
impl BrowserAtlas {
    /// Keep the rendering surface aligned with the browser viewport.
    pub fn resize(&self, width: f64, height: f64) {
        self.0.resize(width, height);
    }
    /// Insert composed text into the focused field.
    pub fn text(&self, value: String) {
        self.0.input(egui::Event::Text(value));
    }
    /// Paste plain text into the focused field.
    pub fn paste(&self, value: String) {
        self.0.input(egui::Event::Paste(value));
    }
    /// Forward a text editing key from the browser's IME element.
    pub fn key(&self, name: &str, pressed: bool, shift: bool, command: bool) {
        if let Some(key) = egui::Key::from_name(name) {
            self.0.input(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers {
                    shift,
                    command,
                    ..Default::default()
                },
            });
        }
    }
}
