//! Display actor implementation with hardware integration
//!
//! Follows the same `*_hw.rs` pattern as other actors.
//! Receives `DisplayState` snapshots via channel and renders
//! the dashboard using ratatui + mousefood.

extern crate alloc;

use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use mousefood::{EmbeddedBackend, EmbeddedBackendConfig};
use ratatui::Terminal;

use crate::gui;
use crate::messages::DisplayState;

/// Display actor wrapping a ratatui Terminal over an embedded-graphics display.
pub struct DisplayActorHw<'a, D>
where
    D: DrawTarget<Color = Rgb565> + 'static,
{
    terminal: Terminal<EmbeddedBackend<'a, D, Rgb565>>,
    state: DisplayState,
}

impl<'a, D> DisplayActorHw<'a, D>
where
    D: DrawTarget<Color = Rgb565> + 'static,
    D::Error: core::fmt::Debug,
{
    /// Create a new display actor from a mutable display reference.
    pub fn new(display: &'a mut D) -> Self {
        let backend = EmbeddedBackend::new(display, EmbeddedBackendConfig::default());
        let terminal = Terminal::new(backend).expect("terminal init");
        let state = DisplayState {
            control_cycle: 0,
            maintenance_ok: true,
            led_state: false,
            maintenance_tick: 0,
            button_pressed: false,
            uptime_secs: 0,
        };
        Self { terminal, state }
    }

    /// Update the display state from a new snapshot.
    pub fn update_state(&mut self, new_state: DisplayState) {
        self.state = new_state;
    }

    /// Render the current state to the display.
    pub fn render(&mut self) {
        let state = self.state;
        let result = self.terminal.draw(|frame| {
            gui::render_dashboard(frame, &state);
        });
        if let Err(_e) = result {
            defmt::warn!("Display render error");
        }
    }
}
