//! RP2040-specific main entry point (Pico 1 & Pico 2)

use embassy_executor::{Executor, Spawner};
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use pico_template::config::*;
use pico_template::define_channels;
use pico_template::messages::{ButtonMessage, MaintenanceMessage};
use pico_template::tasks;
use static_cell::StaticCell;

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

// Define channels for inter-actor communication (owned by RP2040 platform)
define_channels! {
    BUTTON_TO_CONTROL: ButtonMessage, 4;
    MAINTENANCE_TO_CONTROL: MaintenanceMessage, 2;
}

// RP2040 Embassy task wrappers (concrete types required)
#[embassy_executor::task]
async fn control_task_rp2040(
    from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    tasks::control_task(from_button, from_maintenance).await
}

#[embassy_executor::task]
async fn maintenance_task_rp2040(
    led: Output<'static>,
    to_control: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    tasks::maintenance_task(led, to_control).await
}

#[embassy_executor::task]
async fn button_task_rp2040(
    button: Input<'static>,
    to_control: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
) {
    tasks::button_task(button, to_control).await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("RP2040: Initializing hardware");
    let p = embassy_rp::init(Default::default());

    // Initialize GPIO
    let led = match LED_PIN {
        25 => Output::new(p.PIN_25, Level::Low),
        _ => panic!("Unsupported LED pin"),
    };
    let button = match BUTTON_PIN {
        2 => Input::new(p.PIN_2, Pull::Up),
        _ => panic!("Unsupported button pin"),
    };

    // Multi-core task distribution:
    // Core 0: High-priority real-time control task (1kHz)
    defmt::info!("Core 0: Spawning control task");
    spawner
        .spawn(control_task_rp2040(
            &BUTTON_TO_CONTROL,
            &MAINTENANCE_TO_CONTROL,
        ))
        .unwrap();

    // Core 1: Maintenance and button tasks
    defmt::info!("Core 1: Starting secondary executor");
    spawn_core1(p.CORE1, unsafe { &mut CORE1_STACK }, move || {
        let executor = EXECUTOR1.init(Executor::new());
        executor.run(|spawner| {
            defmt::info!("Core 1: Spawning peripheral tasks");
            spawner
                .spawn(maintenance_task_rp2040(led, &MAINTENANCE_TO_CONTROL))
                .unwrap();
            spawner
                .spawn(button_task_rp2040(button, &BUTTON_TO_CONTROL))
                .unwrap();
        });
    });
}
