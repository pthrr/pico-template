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
    #[cfg(feature = "display")] to_display: &'static Channel<
        CriticalSectionRawMutex,
        crate::messages::DisplayState,
        2,
    >,
) {
    defmt::info!("Control actor starting - target 1kHz (1ms period)");

    let mut actor = ControlActorHw::new(
        from_button,
        from_maintenance,
        #[cfg(feature = "display")]
        to_display,
    );

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
    defmt::info!("Maintenance actor starting - 10Hz with ~3s LED toggle");

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

/// Display task (~30Hz) - platform-agnostic
///
/// Receives `DisplayState` snapshots via channel and renders the
/// dashboard to an SPI LCD using ratatui + mousefood.
#[cfg(feature = "display")]
pub async fn display_task<D>(
    display: &mut D,
    from_control: &'static Channel<CriticalSectionRawMutex, crate::messages::DisplayState, 2>,
) where
    D: embedded_graphics::prelude::DrawTarget<Color = embedded_graphics::pixelcolor::Rgb565>
        + 'static,
    D::Error: core::fmt::Debug,
{
    use crate::display_hw::DisplayActorHw;

    defmt::info!("Display actor starting - ~30Hz refresh");

    let mut actor = DisplayActorHw::new(display);

    loop {
        // Drain the channel, keeping only the latest state (latest-wins)
        while let Ok(state) = from_control.try_receive() {
            actor.update_state(state);
        }

        actor.render();

        // ~30Hz refresh period from config
        Timer::after(Duration::from_millis(DISPLAY_REFRESH_MS as u64)).await;
    }
}

/// Bootloader task — reads protocol commands from a byte stream, dispatches
/// to the bootloader actor, writes responses back.
///
/// `R` and `W` are the platform-specific read/write halves of the transport
/// (USB CDC ACM on RP, UART on STM32).
#[cfg(feature = "bootloader")]
pub async fn bootloader_task<F, R, W>(flash: F, mut reader: R, mut writer: W)
where
    F: crate::bootloader::FlashStorage,
    R: embedded_io_async::Read,
    W: embedded_io_async::Write,
{
    use crate::bootloader::protocol::ProtocolParser;
    use crate::bootloader_hw::BootloaderActorHw;

    defmt::info!("Bootloader actor starting");

    let mut actor = BootloaderActorHw::new(flash);
    let mut parser = ProtocolParser::new();
    let mut rx_buf = [0u8; 1];
    let mut tx_buf = [0u8; 128];

    loop {
        match reader.read(&mut rx_buf).await {
            Ok(0) | Err(_) => {
                // Connection lost / EOF — wait and retry
                Timer::after(Duration::from_millis(100)).await;
                continue;
            }
            Ok(_) => {}
        }

        if let Some(cmd) = parser.feed(rx_buf[0]) {
            let payload = parser.binary_payload();
            let resp = actor.process_command(cmd, payload);
            let len = resp.write_to(&mut tx_buf);
            let _ = writer.write_all(&tx_buf[..len]).await;
        }
    }
}

// Platform-specific Embassy task wrappers defined in main_*.rs files
// (Embassy tasks cannot be generic, so each platform defines its own concrete wrappers)
