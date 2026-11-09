use super::*;
use maplibre::{hud::NavigationHudState, render::camera::EdgeInsets};

/// A 600×400 map that records what the controls ask of it.
#[cfg(test)]
#[derive(Default)]
struct Recorder {
    navigations: usize,
    zooms: Vec<(egui::Vec2, f64)>,
}

impl ControlledMap for Recorder {
    fn view_size(&self) -> [f64; 2] {
        [600.0, 400.0]
    }
    fn zoom_and_bearing(&self) -> (f64, cgmath::Rad<f64>) {
        (5.0, cgmath::Rad(0.0))
    }
    fn is_over_hud(&self, _: &NavigationState, pixel: egui::Vec2) -> bool {
        let position = Vector2::new(f64::from(pixel.x), f64::from(pixel.y));
        NavigationHudState::hit_test(position, 600.0, 400.0, &EdgeInsets::default()).is_some()
    }
    fn navigate(&mut self, _: &mut NavigationState, _: Duration) -> bool {
        self.navigations = self.navigations.wrapping_add(1);
        false
    }
    fn zoom(&mut self, pixel: egui::Vec2, delta: f64) -> bool {
        self.zooms.push((pixel, delta));
        true
    }
}

/// Centre of the HUD's zoom-in slot for a 600×400 viewport.
#[cfg(test)]
fn zoom_in_button() -> egui::Vec2 {
    let origin = NavigationHudState::panel_origin(600.0, 400.0, &EdgeInsets::default());
    egui::vec2(origin.x as f32 + 19.0, origin.y as f32 + 88.0)
}

#[test]
fn hud_buttons_own_their_clicks_and_zoom_about_the_view_centre() {
    let mut map = Recorder::default();
    let mut controls = MapControls::default();
    let button = zoom_in_button();
    let press = Pointer {
        pixel: Some(button),
        pressed: true,
        released: false,
        hovered: true,
    };
    assert!(controls.pointer(&map, press));
    let release = Pointer {
        pressed: false,
        released: true,
        ..press
    };
    assert!(controls.pointer(&map, release));
    assert!(controls.advance(&mut map));
    assert_eq!(map.navigations, 1);
    let [(pixel, delta)] = map.zooms.as_slice() else {
        panic!("one zoom request");
    };
    assert_eq!(*pixel, egui::vec2(300.0, 200.0));
    assert!(*delta > 0.0);
    // Nothing pending: the controls settle and stop asking for frames.
    assert!(!controls.advance(&mut map));
}

#[test]
fn map_pixels_away_from_the_hud_stay_with_map_gestures() {
    let map = Recorder::default();
    let mut controls = MapControls::default();
    let press = Pointer {
        pixel: Some(egui::vec2(100.0, 300.0)),
        pressed: true,
        released: false,
        hovered: true,
    };
    assert!(!controls.pointer(&map, press));
    assert!(!controls.pointer(&map, Pointer::default()));
}

#[test]
fn a_click_pressed_and_released_within_one_frame_still_activates_the_button() {
    let mut map = Recorder::default();
    let mut controls = MapControls::default();
    let click = Pointer {
        pixel: Some(zoom_in_button()),
        pressed: true,
        released: true,
        hovered: true,
    };
    assert!(controls.pointer(&map, click));
    controls.advance(&mut map);
    assert_eq!(map.zooms.len(), 1);
    // The button is released, so later pointer motion over the map is not captured.
    let elsewhere = Pointer {
        pixel: Some(egui::vec2(100.0, 300.0)),
        ..Pointer::default()
    };
    assert!(!controls.pointer(&map, elsewhere));
}

#[test]
fn a_manual_turn_leaves_auto_tilt_for_manual_three_dimensions() {
    let mut controls = MapControls::default();
    controls.state.auto_tilt = true;
    controls.state.three_dimensional = false;
    controls.manual_turn();
    assert!(controls.state.three_dimensional);
    assert!(!controls.state.auto_tilt);
}
