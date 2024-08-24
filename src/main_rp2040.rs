//! RP2040-specific main entry point (Pico 1 & Pico 2)
//!
//! Same `SysML` use case on all platforms: button → control ← maintenance (LED).

use embassy_executor::{Executor, Spawner};
use embassy_rp::config::Config;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use pico_template::config::{BUTTON_PIN, LED_PIN};
use pico_template::generated::channels::{BUTTON_TO_CONTROL, MAINTENANCE_TO_CONTROL};
use pico_template::messages::{ButtonMessage, MaintenanceMessage};
use pico_template::tasks;
use static_cell::StaticCell;

static CORE1_STACK: StaticCell<Stack<4096>> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

#[embassy_executor::task]
async fn control_task_rp2040(
    from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    tasks::control_task(from_button, from_maintenance).await;
}

#[embassy_executor::task]
async fn maintenance_task_rp2040(
    led: Output<'static>,
    to_control: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    tasks::maintenance_task(led, to_control).await;
}

#[embassy_executor::task]
async fn button_task_rp2040(
    button: Input<'static>,
    to_control: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
) {
    tasks::button_task(button, to_control).await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("RP2040: Initializing hardware");
    let p = embassy_rp::init(Config::default());

    let led = match LED_PIN {
        25 => Output::new(p.PIN_25, Level::Low),
        _ => panic!("Unsupported LED pin"),
    };
    let button = match BUTTON_PIN {
        2 => Input::new(p.PIN_2, Pull::Up),
        _ => panic!("Unsupported button pin"),
    };

    defmt::info!("Core 0: Spawning control task");
    spawner
        .spawn(control_task_rp2040(
            &BUTTON_TO_CONTROL,
            &MAINTENANCE_TO_CONTROL,
        ))
        .expect("spawn control");

    spawn_core1(p.CORE1, CORE1_STACK.init(Stack::new()), move || {
        let executor = EXECUTOR1.init(Executor::new());
        executor.run(|spawner| {
            defmt::info!("Core 1: Spawning button and maintenance tasks");
            spawner
                .spawn(maintenance_task_rp2040(led, &MAINTENANCE_TO_CONTROL))
                .expect("spawn maintenance");
            spawner
                .spawn(button_task_rp2040(button, &BUTTON_TO_CONTROL))
                .expect("spawn button");
        });
    });
}
