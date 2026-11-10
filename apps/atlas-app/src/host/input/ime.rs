//! The text-input state the page's IME bridge observes.

/// The last IME state written to the page. Each write is a DOM mutation the page reacts
/// to, so the state is written only when it changes, not on every frame.
#[derive(Debug, Default)]
pub(crate) struct ImeFlag(Option<bool>);

impl ImeFlag {
    /// Record whether egui wants text input; returns the attribute value to write when it
    /// differs from the one last written.
    pub(crate) fn update(&mut self, active: bool) -> Option<&'static str> {
        if self.0 == Some(active) {
            return None;
        }
        self.0 = Some(active);
        Some(if active { "true" } else { "false" })
    }
}

#[cfg(test)]
mod tests;
