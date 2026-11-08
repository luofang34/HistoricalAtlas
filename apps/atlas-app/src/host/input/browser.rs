use egui::{Event, Modifiers, PointerButton};
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    window::Window,
};

pub(crate) struct Input {
    raw: egui::RawInput,
    pointer: egui::Pos2,
    started: web_time::Instant,
    modifiers: Modifiers,
}

impl Input {
    pub(crate) fn new(_context: egui::Context, _window: &Window, texture_limit: usize) -> Self {
        Self {
            raw: egui::RawInput {
                max_texture_side: Some(texture_limit),
                ..Default::default()
            },
            pointer: egui::Pos2::ZERO,
            started: web_time::Instant::now(),
            modifiers: Modifiers::default(),
        }
    }

    pub(crate) fn inject(&mut self, event: Event) {
        self.raw.events.push(event);
    }

    pub(crate) fn event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let position = position.to_logical::<f32>(window.scale_factor());
                self.pointer = egui::pos2(position.x, position.y);
                self.inject(Event::PointerMoved(self.pointer));
            }
            WindowEvent::CursorLeft { .. } => self.inject(Event::PointerGone),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => PointerButton::Primary,
                    MouseButton::Right => PointerButton::Secondary,
                    MouseButton::Middle => PointerButton::Middle,
                    _ => return false,
                };
                self.inject(Event::PointerButton {
                    pos: self.pointer,
                    button,
                    pressed: *state == ElementState::Pressed,
                    modifiers: self.modifiers,
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (unit, delta) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (egui::MouseWheelUnit::Line, egui::vec2(*x, *y))
                    }
                    MouseScrollDelta::PixelDelta(point) => {
                        let point = point.to_logical::<f32>(window.scale_factor());
                        (egui::MouseWheelUnit::Point, egui::vec2(point.x, point.y))
                    }
                };
                self.inject(Event::MouseWheel {
                    unit,
                    delta,
                    modifiers: self.modifiers,
                    phase: egui::TouchPhase::Move,
                });
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.modifiers = Modifiers {
                    alt: state.alt_key(),
                    ctrl: state.control_key(),
                    shift: state.shift_key(),
                    mac_cmd: state.super_key(),
                    command: state.super_key() || state.control_key(),
                };
            }
            WindowEvent::KeyboardInput { event, .. } => self.keyboard(event),
            WindowEvent::Focused(focused) => self.raw.focused = *focused,
            _ => return false,
        }
        true
    }

    fn keyboard(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{Key, NamedKey};
        let name = match &event.logical_key {
            Key::Character(value) => value.as_str(),
            Key::Named(key) => match key {
                NamedKey::Enter => "Enter",
                NamedKey::Backspace => "Backspace",
                NamedKey::Delete => "Delete",
                NamedKey::ArrowLeft => "ArrowLeft",
                NamedKey::ArrowRight => "ArrowRight",
                NamedKey::ArrowUp => "ArrowUp",
                NamedKey::ArrowDown => "ArrowDown",
                NamedKey::Tab => "Tab",
                NamedKey::Home => "Home",
                NamedKey::End => "End",
                NamedKey::Escape => "Escape",
                _ => "",
            },
            _ => "",
        };
        if let Some(key) = egui::Key::from_name(name) {
            self.inject(Event::Key {
                key,
                physical_key: None,
                pressed: event.state.is_pressed(),
                repeat: event.repeat,
                modifiers: self.modifiers,
            });
        }
        if event.state.is_pressed()
            && !self.modifiers.command
            && let Some(text) = &event.text
            && text.chars().all(|ch| !ch.is_control())
        {
            self.inject(Event::Text(text.to_string()));
        }
    }

    pub(crate) fn take(&mut self, window: &Window) -> egui::RawInput {
        let size = window.inner_size().to_logical::<f32>(window.scale_factor());
        self.raw.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(size.width, size.height),
        ));
        self.raw.time = Some(self.started.elapsed().as_secs_f64());
        self.raw
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(window.scale_factor() as f32);
        self.raw.take()
    }

    pub(crate) fn output(
        &mut self,
        window: &Window,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        output: egui::PlatformOutput,
    ) {
        window.set_cursor(match output.cursor_icon {
            egui::CursorIcon::Text => winit::window::CursorIcon::Text,
            egui::CursorIcon::PointingHand => winit::window::CursorIcon::Pointer,
            _ => winit::window::CursorIcon::Default,
        });
        for command in output.commands {
            if let egui::OutputCommand::OpenUrl(url) = command
                && let Some(browser) = web_sys::window()
            {
                browser.open_with_url_and_target(&url.url, "_blank").ok();
            }
        }
        if let Some(input) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("ime"))
        {
            input
                .set_attribute(
                    "data-active",
                    if output.ime.is_some() {
                        "true"
                    } else {
                        "false"
                    },
                )
                .ok();
        }
    }
}
