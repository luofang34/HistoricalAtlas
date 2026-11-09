//! MapLibre's shared navigation HUD: compass, 2D/3D and Auto-Tilt, zoom, and Home.

use crate::scene::renderer::MapRenderer;
use cgmath::Vector2;
use maplibre::navigation::NavigationState;
use web_time::{Duration, Instant};

/// Longest step a settle animation advances in one frame, so a stalled frame
/// does not jump the camera to the end of the animation.
const MAXIMUM_STEP: Duration = Duration::from_millis(100);

/// Assumed frame interval when the controls start moving after being idle.
const FIRST_STEP: Duration = Duration::from_millis(16);

/// The map operations the controls need, so their routing is testable without a GPU.
pub(super) trait ControlledMap {
    /// Viewport size in logical pixels.
    fn view_size(&self) -> [f64; 2];
    /// Current zoom and bearing.
    fn zoom_and_bearing(&self) -> (f64, cgmath::Rad<f64>);
    /// Whether a logical pixel lies on the HUD.
    fn is_over_hud(&self, controls: &NavigationState, pixel: egui::Vec2) -> bool;
    /// Advance the controls; returns whether they still move the camera.
    fn navigate(&mut self, controls: &mut NavigationState, dt: Duration) -> bool;
    /// Zoom by `delta` levels about a logical pixel.
    fn zoom(&mut self, pixel: egui::Vec2, delta: f64) -> bool;
}

impl ControlledMap for MapRenderer {
    fn view_size(&self) -> [f64; 2] {
        MapRenderer::view_size(self)
    }
    fn zoom_and_bearing(&self) -> (f64, cgmath::Rad<f64>) {
        MapRenderer::zoom_and_bearing(self)
    }
    fn is_over_hud(&self, controls: &NavigationState, pixel: egui::Vec2) -> bool {
        MapRenderer::is_over_hud(self, controls, pixel)
    }
    fn navigate(&mut self, controls: &mut NavigationState, dt: Duration) -> bool {
        MapRenderer::navigate(self, controls, dt)
    }
    fn zoom(&mut self, pixel: egui::Vec2, delta: f64) -> bool {
        MapRenderer::zoom(self, pixel, delta)
    }
}

/// Pointer facts for one frame over the map, in logical pixels from its top-left corner.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Pointer {
    pub(super) pixel: Option<egui::Vec2>,
    pub(super) pressed: bool,
    pub(super) released: bool,
    pub(super) hovered: bool,
}

impl Pointer {
    pub(super) fn read(ui: &egui::Ui, response: &egui::Response) -> Self {
        let (pressed, released) = ui.input(|input| {
            (
                input.pointer.primary_pressed(),
                input.pointer.primary_released(),
            )
        });
        Self {
            pixel: response
                .interact_pointer_pos()
                .or_else(|| response.hover_pos())
                .map(|position| position - response.rect.min),
            pressed,
            released,
            hovered: response.hovered(),
        }
    }
}

/// Navigation controls and the time of the last frame they advanced.
#[derive(Default)]
pub(super) struct MapControls {
    state: NavigationState,
    last_frame: Option<Instant>,
}

impl MapControls {
    /// Route the pointer to the HUD; returns whether the HUD owns this interaction,
    /// so map dragging, zooming and picking must not also act on it.
    pub(super) fn pointer(&mut self, map: &dyn ControlledMap, pointer: Pointer) -> bool {
        let Some(pixel) = pointer.pixel else {
            return self.state.pressed_button.is_some();
        };
        let cursor = Vector2::new(f64::from(pixel.x), f64::from(pixel.y));
        let [width, height] = map.view_size();
        self.state.process_cursor_moved(cursor, width, height);
        // A quick click can press and release within one GUI frame; both must reach the HUD
        // or the button stays held and captures the pointer.
        let pressed = pointer.pressed
            && pointer.hovered
            && self.state.process_hud_press(cursor, width, height);
        if pointer.released && self.state.pressed_button.is_some() {
            let (zoom, bearing) = map.zoom_and_bearing();
            self.state.process_hud_release(zoom, bearing);
            return true;
        }
        pressed || self.state.pressed_button.is_some() || map.is_over_hud(&self.state, pixel)
    }

    /// Note a pan or zoom made directly on the map, which ends HUD-driven settling.
    pub(super) fn manual_move(&mut self) {
        self.state.on_manual_pan_or_zoom();
    }

    /// Note a bearing or pitch turn made directly on the map, which leaves Auto-Tilt for
    /// manual 3D as the shared HUD expects.
    pub(super) fn manual_turn(&mut self) {
        self.state.on_manual_bearing();
        self.state.on_manual_tilt();
    }

    /// Advance settle animations and HUD zoom requests; returns whether to draw again.
    pub(super) fn advance(&mut self, map: &mut dyn ControlledMap) -> bool {
        let now = Instant::now();
        let step = self
            .last_frame
            .map_or(FIRST_STEP, |last| now.duration_since(last))
            .min(MAXIMUM_STEP);
        let mut moving = map.navigate(&mut self.state, step);
        if let Some(delta) = self.state.take_pending_smooth_zoom() {
            let [width, height] = map.view_size();
            map.zoom(egui::vec2(width as f32 / 2.0, height as f32 / 2.0), delta);
            moving = true;
        }
        self.last_frame = moving.then_some(now);
        moving
    }
}

#[cfg(test)]
mod tests;
