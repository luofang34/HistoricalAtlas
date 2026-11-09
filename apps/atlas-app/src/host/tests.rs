use super::redraws_after;
use winit::event::WindowEvent;

#[test]
fn a_drawn_frame_does_not_request_the_next() {
    assert!(!redraws_after(&WindowEvent::RedrawRequested, true));
}

#[test]
fn input_that_egui_needs_redrawn_requests_a_frame() {
    assert!(redraws_after(&WindowEvent::Focused(true), true));
    assert!(!redraws_after(&WindowEvent::Focused(true), false));
}
