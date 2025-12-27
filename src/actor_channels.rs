//! Macro for defining actor communication channels
//!
//! This eliminates boilerplate when setting up channels between actors.

/// Define static channels for inter-actor communication
///
/// # Usage
/// ```ignore
/// define_channels! {
///     BUTTON_TO_CONTROL: ButtonMessage, 4;
///     MAINTENANCE_TO_CONTROL: MaintenanceMessage, 2;
/// }
/// ```
#[macro_export]
macro_rules! define_channels {
    ($($name:ident: $msg_type:ty, $capacity:expr);+ $(;)?) => {
        $(
            static $name: embassy_sync::channel::Channel<
                embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
                $msg_type,
                $capacity
            > = embassy_sync::channel::Channel::new();
        )+
    };
}
