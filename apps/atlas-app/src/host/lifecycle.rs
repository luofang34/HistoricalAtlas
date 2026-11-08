use super::{Host, HostError};
use crate::catalog::Atlas;
use crate::overlay::OverlayInput;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

enum Signal {
    Ready,
    Repaint(Duration),
    #[cfg(target_arch = "wasm32")]
    Input(egui::Event),
    #[cfg(target_arch = "wasm32")]
    Resize(f64, f64),
    #[cfg(target_arch = "wasm32")]
    Overlay(OverlayInput),
}

struct Lifecycle {
    atlas: Option<Atlas>,
    host: Option<Host>,
    pending: Rc<RefCell<Option<Result<Host, HostError>>>>,
    proxy: EventLoopProxy<Signal>,
    error: Option<HostError>,
    repaint_at: Option<Instant>,
    pending_overlay: Option<OverlayInput>,
    #[cfg(target_arch = "wasm32")]
    canvas: web_sys::HtmlCanvasElement,
}

impl Lifecycle {
    fn ready(&mut self, event_loop: &ActiveEventLoop) {
        let result = self.pending.borrow_mut().take();
        match result {
            Some(Ok(mut host)) => {
                if let Some(input) = self.pending_overlay.take() {
                    host.application.load_overlay(input);
                }
                let proxy = self.proxy.clone();
                host.context.set_request_repaint_callback(move |info| {
                    proxy.send_event(Signal::Repaint(info.delay)).ok();
                });
                host.window.request_redraw();
                self.host = Some(host);
            }
            Some(Err(error)) => self.fail(event_loop, error),
            None => {}
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: HostError) {
        tracing::error!(%error, "atlas window failed");
        #[cfg(target_arch = "wasm32")]
        if let Some(status) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("status"))
        {
            status.set_text_content(Some(&error.to_string()));
        }
        self.error = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler<Signal> for Lifecycle {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(atlas) = self.atlas.take() else {
            return;
        };
        let attributes = Window::default_attributes()
            .with_title("Historical Atlas")
            .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0));
        #[cfg(target_arch = "wasm32")]
        let attributes = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes
                .with_canvas(Some(self.canvas.clone()))
                .with_prevent_default(true)
                .with_inner_size(winit::dpi::LogicalSize::new(
                    self.canvas.client_width().max(1),
                    self.canvas.client_height().max(1),
                ))
        };
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error.into());
                return;
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            *self.pending.borrow_mut() =
                Some(tokio::runtime::Handle::current().block_on(Host::new(window, atlas)));
            self.proxy.send_event(Signal::Ready).ok();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let pending = self.pending.clone();
            let proxy = self.proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = Host::new(window, atlas).await;
                *pending.borrow_mut() = Some(result);
                proxy.send_event(Signal::Ready).ok();
            });
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Signal) {
        match event {
            Signal::Ready => self.ready(event_loop),
            #[cfg(target_arch = "wasm32")]
            Signal::Input(event) => {
                if let Some(host) = &mut self.host {
                    host.input.inject(event);
                    host.window.request_redraw();
                }
            }
            #[cfg(target_arch = "wasm32")]
            Signal::Resize(width, height) => {
                if let Some(host) = &self.host {
                    let requested = host
                        .window
                        .request_inner_size(winit::dpi::LogicalSize::new(width, height));
                    if requested.is_some() {
                        host.window.request_redraw();
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            Signal::Overlay(input) => {
                if let Some(host) = &mut self.host {
                    host.application.load_overlay(input);
                    host.window.request_redraw();
                } else {
                    self.pending_overlay = Some(input);
                }
            }
            Signal::Repaint(delay) => {
                if let Some(deadline) = Instant::now().checked_add(delay) {
                    self.repaint_at =
                        Some(self.repaint_at.map_or(deadline, |old| old.min(deadline)));
                    if let Some(at) = self.repaint_at {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(at));
                    }
                }
            }
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            self.repaint_at = None;
            event_loop.set_control_flow(ControlFlow::Wait);
            if let Some(host) = &self.host {
                host.window.request_redraw();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(host) = &mut self.host
            && host.window.id() == id
        {
            host.event(event_loop, event);
        }
    }
}

/// Run the desktop window while an explicit Tokio runtime is entered.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_blocking(atlas: Atlas, overlay: Option<OverlayInput>) -> Result<(), HostError> {
    let event_loop = EventLoop::<Signal>::with_user_event().build()?;
    let mut lifecycle = Lifecycle {
        atlas: Some(atlas),
        host: None,
        pending: Rc::new(RefCell::new(None)),
        proxy: event_loop.create_proxy(),
        error: None,
        repaint_at: None,
        pending_overlay: overlay,
    };
    event_loop.run_app(&mut lifecycle)?;
    match lifecycle.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Start a browser event loop attached to the supplied canvas.
#[cfg(target_arch = "wasm32")]
pub fn spawn(
    atlas: Atlas,
    canvas: web_sys::HtmlCanvasElement,
) -> Result<BrowserControl, HostError> {
    use winit::platform::web::EventLoopExtWebSys;
    let event_loop = EventLoop::<Signal>::with_user_event().build()?;
    let control = BrowserControl(event_loop.create_proxy());
    let lifecycle = Lifecycle {
        atlas: Some(atlas),
        host: None,
        pending: Rc::new(RefCell::new(None)),
        proxy: event_loop.create_proxy(),
        error: None,
        repaint_at: None,
        pending_overlay: None,
        canvas,
    };
    event_loop.spawn_app(lifecycle);
    Ok(control)
}

/// Browser text and IME events routed to the focused shared UI widget.
#[cfg(target_arch = "wasm32")]
pub struct BrowserControl(EventLoopProxy<Signal>);

#[cfg(target_arch = "wasm32")]
impl BrowserControl {
    /// Queue an external package or loading error, including during GPU startup.
    pub fn overlay(&self, input: OverlayInput) {
        self.0.send_event(Signal::Overlay(input)).ok();
    }

    /// Queue an input event and wake the browser application.
    pub fn input(&self, event: egui::Event) {
        self.0.send_event(Signal::Input(event)).ok();
    }

    /// Resize the window to the canvas's CSS dimensions.
    pub fn resize(&self, width: f64, height: f64) {
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            self.0.send_event(Signal::Resize(width, height)).ok();
        }
    }
}
