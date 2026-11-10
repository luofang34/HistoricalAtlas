#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "only the browser host has an IME bridge")
)]
mod ime;
#[cfg(target_arch = "wasm32")]
pub(super) use browser::Input;

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct Input(egui_winit::State);

#[cfg(not(target_arch = "wasm32"))]
impl Input {
    pub(super) fn new(
        context: egui::Context,
        window: &winit::window::Window,
        texture_limit: usize,
    ) -> Self {
        Self(egui_winit::State::new(
            context,
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(texture_limit),
        ))
    }

    pub(super) fn event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        self.0.on_window_event(window, event).repaint
    }

    pub(super) fn take(&mut self, window: &winit::window::Window) -> egui::RawInput {
        self.0.take_egui_input(window)
    }

    pub(super) fn output(
        &mut self,
        window: &winit::window::Window,
        event_loop: &winit::event_loop::ActiveEventLoop,
        output: egui::PlatformOutput,
    ) {
        self.0
            .handle_platform_output_with_event_loop(window, event_loop, output);
    }
}
