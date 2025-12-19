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
