//! Message types for inter-actor communication

/// Messages from button actor to control actor
#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum ButtonMessage {
    /// Button was pressed
    Pressed,
    /// Button was released
    Released,
}

/// Messages from maintenance actor to control actor
#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct MaintenanceMessage {
    /// System health status
    pub system_ok: bool,
    /// Current LED state
    pub led_state: bool,
    /// Current tick count
    pub tick_count: i32,
}

/// Snapshot of system state for display rendering
#[cfg(feature = "display")]
#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct DisplayState {
    /// Control loop cycle counter
    pub control_cycle: i32,
    /// Maintenance subsystem health
    pub maintenance_ok: bool,
    /// Current LED state
    pub led_state: bool,
    /// Maintenance tick counter
    pub maintenance_tick: i32,
    /// Button currently pressed
    pub button_pressed: bool,
    /// System uptime in seconds
    pub uptime_secs: u32,
}
