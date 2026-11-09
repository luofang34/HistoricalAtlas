//! Window lifecycle and shared GPU presentation for native and browser hosts.

use crate::{application::Application, catalog::Atlas, error::AppError};
use egui_wgpu::winit::Painter;
use std::{num::NonZeroU32, sync::Arc};
use winit::{event::WindowEvent, event_loop::ActiveEventLoop, window::Window};

mod input;
mod lifecycle;
#[cfg(test)]
mod tests;
#[cfg(not(target_arch = "wasm32"))]
pub use lifecycle::run_blocking;
#[cfg(target_arch = "wasm32")]
pub use lifecycle::{BrowserControl, spawn};

/// Application window initialization or execution failure.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// A publication font cannot be loaded.
    #[error(transparent)]
    Font(#[from] crate::fonts::FontError),
    /// A static browser resource could not be loaded.
    #[error("atlas asset: {0}")]
    Resource(#[from] maplibre::io::source_client::SourceFetchError),
    /// The operating system could not initialize the event loop.
    #[error("window event loop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    /// The window could not be created.
    #[error("window creation: {0}")]
    Window(#[from] winit::error::OsError),
    /// GPU surface or device creation failed.
    #[error("GPU initialization: {0}")]
    Gpu(#[from] egui_wgpu::WgpuError),
    /// The window has no initialized GPU state.
    #[error("window GPU state is unavailable")]
    MissingGpu,
    /// Application evidence or renderer failed.
    #[error(transparent)]
    Application(#[from] AppError),
}

struct Host {
    application: Application,
    context: egui::Context,
    input: input::Input,
    painter: Painter,
    window: Arc<Window>,
}

impl Host {
    async fn new(window: Arc<Window>, atlas: Atlas) -> Result<Self, HostError> {
        let context = egui::Context::default();
        context.set_visuals(egui::Visuals::dark());
        #[cfg(not(target_arch = "wasm32"))]
        crate::application::configure_fonts_blocking(&context)?;
        #[cfg(target_arch = "wasm32")]
        crate::resources::fonts(&context).await?;
        let mut painter = Painter::new(
            context.clone(),
            Default::default(),
            false,
            Default::default(),
        )
        .await;
        painter
            .set_window(egui::ViewportId::ROOT, Some(window.clone()))
            .await?;
        let gpu = painter.render_state().ok_or(HostError::MissingGpu)?;
        let input = input::Input::new(
            context.clone(),
            &window,
            gpu.device.limits().max_texture_dimension_2d as usize,
        );
        let mut application = Application::new(atlas)?;
        application.set_basemap(crate::scene::basemap::load().await?);
        application.attach(gpu)?;
        Ok(Self {
            application,
            context,
            input,
            painter,
            window,
        })
    }

    fn event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let repaint = redraws_after(&event, self.input.event(&self.window, &event));
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.painter
                        .on_window_resized(egui::ViewportId::ROOT, width, height);
                    self.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.draw(event_loop),
            _ => {}
        }
        if repaint {
            self.window.request_redraw();
        }
    }

    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let input = self.input.take(&self.window);
        let mut output = self.context.run_ui(input, |ui| self.application.ui(ui));
        self.input
            .output(&self.window, event_loop, output.platform_output);
        let shapes = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        self.window.pre_present_notify();
        self.painter.paint_and_update_textures(
            egui::ViewportId::ROOT,
            output.pixels_per_point,
            [0.03, 0.05, 0.06, 1.0],
            &shapes,
            &mut output.textures_delta,
            Vec::new(),
            &self.window,
        );
    }
}

/// Whether a window event needs another frame. egui marks every event that may
/// change the view as needing a repaint, including the redraw itself; answering a
/// redraw with another one would render continuously, so further frames after a
/// redraw come only from egui's repaint requests and the map's own redraw needs.
fn redraws_after(event: &WindowEvent, egui_repaint: bool) -> bool {
    egui_repaint && !matches!(event, WindowEvent::RedrawRequested)
}
