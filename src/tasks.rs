//! Common task implementations shared across all platforms

use crate::button_hw::ButtonActorHw;
use crate::config::*;
use crate::control_hw::ControlActorHw;
use crate::hal::{InputPin, OutputPin};
use crate::maintenance_hw::MaintenanceActorHw;
use crate::messages::{ButtonMessage, MaintenanceMessage};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};

/// Control task (1kHz) - platform-agnostic
pub async fn control_task(
    from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    defmt::info!("Control actor starting - target 1kHz (1ms period)");

    let mut actor = ControlActorHw::new(from_button, from_maintenance);

    loop {
        let loop_start = Instant::now();

        actor.step();

        // Target 1kHz (1ms period)
        let elapsed = Instant::now() - loop_start;
        let target_period = Duration::from_millis(CONTROL_PERIOD_MS as u64);
        if elapsed < target_period {
            Timer::after(target_period - elapsed).await;
        } else {
            defmt::info!(
                "Control: Missed deadline by {}us",
                (elapsed - target_period).as_micros()
            );
        }
    }
}

/// Maintenance task (10Hz) - platform-agnostic
pub async fn maintenance_task<O: OutputPin>(
    led: O,
    to_control: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    defmt::info!("Maintenance actor starting - 10Hz with 1s LED toggle");

    let mut actor = MaintenanceActorHw::new(led, to_control);

    loop {
        actor.step();

        // 10Hz = 100ms period
        Timer::after(Duration::from_millis(MAINTENANCE_PERIOD_MS as u64)).await;
    }
}

/// Button task (interrupt-driven) - platform-agnostic
pub async fn button_task<I: InputPin>(
    button_pin: I,
    to_control: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
) {
    defmt::info!("Button actor starting - interrupt-driven with debouncing");

    let mut actor = ButtonActorHw::new(button_pin, to_control);

    loop {
        Timer::after(Duration::from_millis(BUTTON_DEBOUNCE_MS as u64)).await;
        actor.step();
    }
}

// Platform-specific Embassy task wrappers defined in main_*.rs files
// (Embassy tasks cannot be generic, so each platform defines its own concrete wrappers)
