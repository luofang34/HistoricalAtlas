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

    /// Turn the camera by a right- or Ctrl-drag: horizontal travel turns the bearing and
    /// vertical travel the pitch, at the rate MapLibre's window host uses; returns whether
    /// the camera moved.
    pub fn orbit(
        &mut self,
        delta: egui::Vec2,
    ) -> Result<bool, maplibre::render::view_state::NavigationError> {
        let camera = self.map.view_state().camera();
        let bearing = cgmath::Deg::from(camera.get_bearing());
        let pitch = cgmath::Deg::from(camera.get_pitch());
        let maximum = cgmath::Deg::from(camera.max_pitch());
        let turned = cgmath::Deg(bearing.0 - f64::from(delta.x) * ORBIT_DEGREES_PER_PIXEL);
        let tilted = cgmath::Deg(
            (pitch.0 - f64::from(delta.y) * ORBIT_DEGREES_PER_PIXEL).clamp(0.0, maximum.0),
        );
        if turned == bearing && tilted == pitch {
            return Ok(false);
        }
        self.map
            .jump_to(maplibre::render::view_state::CameraOptions {
                bearing: Some(turned),
                pitch: Some(tilted),
                ..Default::default()
            })?;
        Ok(true)
    }
}

/// Degrees of bearing or pitch per logical pixel of an orbit drag.
const ORBIT_DEGREES_PER_PIXEL: f64 = 0.35;
