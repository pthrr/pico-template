//! Arduino UNO Q main — same `SysML` use case as RP2040 (button, maintenance/LED, control).
//!
//! On-board: RGB LED 3 green (PH11, active-low). There is no on-board user button on the
//! STM32 — use Arduino header D7 (PB2) to GND, or any supported `button_pin` in CUE.

use embassy_executor::Spawner;
use embassy_stm32::Config;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use pico_template::config::{BUTTON_PIN, LED_PIN};
use pico_template::generated::channels::{BUTTON_TO_CONTROL, MAINTENANCE_TO_CONTROL};
use pico_template::hal::OutputPin;
use pico_template::messages::{ButtonMessage, MaintenanceMessage};
use pico_template::tasks;

/// PH11 user LED is active-low; maintenance actor drives logical on/off.
struct ActiveLowLed<O>(O);

impl<O: OutputPin> OutputPin for ActiveLowLed<O> {
    fn set_high(&mut self) {
        self.0.set_low();
    }
    fn set_low(&mut self) {
        self.0.set_high();
    }
}

#[embassy_executor::task]
async fn control_task_unoq(
    from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    tasks::control_task(from_button, from_maintenance).await;
}

#[embassy_executor::task]
async fn maintenance_task_unoq(
    led: ActiveLowLed<Output<'static>>,
    to_control: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    tasks::maintenance_task(led, to_control).await;
}

#[embassy_executor::task]
async fn button_task_unoq(
    button: Input<'static>,
    to_control: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
) {
    tasks::button_task(button, to_control).await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("UNO Q: Initializing hardware");
    let p = embassy_stm32::init(Config::default());

    let led_raw = match LED_PIN {
        11 => Output::new(p.PH11, Level::High, Speed::Low),
        _ => panic!("Unsupported LED pin"),
    };
    let led = ActiveLowLed(led_raw);

    let button = match BUTTON_PIN {
        7 => Input::new(p.PB2, Pull::Up), // Arduino D7 (typical Button tutorial wiring)
        _ => panic!("Unsupported button pin (unoq: use 7 = D7/PB2)"),
    };

    defmt::info!("UNO Q: Spawning control (1 kHz)");
    spawner
        .spawn(control_task_unoq(
            &BUTTON_TO_CONTROL,
            &MAINTENANCE_TO_CONTROL,
        ))
        .expect("spawn control");

    defmt::info!("UNO Q: Spawning maintenance and button");
    spawner
        .spawn(maintenance_task_unoq(led, &MAINTENANCE_TO_CONTROL))
        .expect("spawn maintenance");
    spawner
        .spawn(button_task_unoq(button, &BUTTON_TO_CONTROL))
        .expect("spawn button");
}
