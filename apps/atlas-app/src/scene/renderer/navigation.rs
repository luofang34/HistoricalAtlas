//! The shared MapLibre navigation controls, driven through the headless map.

use cgmath::Vector2;
use maplibre::navigation::NavigationState;

use super::MapRenderer;

impl MapRenderer {
    /// Advance the controls by `dt`; returns whether they still move the camera.
    pub fn navigate(&mut self, controls: &mut NavigationState, dt: std::time::Duration) -> bool {
        self.map.navigate(controls, dt)
    }

    /// Viewport size in the logical pixels the HUD is laid out in.
    pub fn view_size(&self) -> [f64; 2] {
        let view = self.map.view_state();
        [view.width(), view.height()]
    }

    /// Whether a logical pixel lies on the navigation HUD.
    pub fn is_over_hud(&self, controls: &NavigationState, pixel: egui::Vec2) -> bool {
        controls.is_over_hud(
            self.map.view_state(),
            Vector2::new(f64::from(pixel.x), f64::from(pixel.y)),
        )
    }

    /// Current zoom and bearing, which HUD button releases act on.
    pub fn zoom_and_bearing(&self) -> (f64, cgmath::Rad<f64>) {
        let view = self.map.view_state();
        (view.zoom().value(), view.camera().get_bearing())
    }
}
