//! STM32U585-specific main entry point

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals::{PC7, PC13};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use pico_template::define_channels;
use pico_template::messages::{ButtonMessage, MaintenanceMessage};
use pico_template::tasks;

// Define channels for inter-actor communication (owned by STM32 platform)
define_channels! {
    BUTTON_TO_CONTROL: ButtonMessage, 4;
    MAINTENANCE_TO_CONTROL: MaintenanceMessage, 2;
}

// STM32 Embassy task wrappers (concrete types required)
#[embassy_executor::task]
async fn control_task_stm32(
    from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    tasks::control_task(from_button, from_maintenance).await
}

#[embassy_executor::task]
async fn maintenance_task_stm32(
    led: Output<'static, PC7>,
    to_control: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    tasks::maintenance_task(led, to_control).await
}

#[embassy_executor::task]
async fn button_task_stm32(
    button: Input<'static, PC13>,
    to_control: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
) {
    tasks::button_task(button, to_control).await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("STM32U585: Initializing hardware");
    let p = embassy_stm32::init(Default::default());

    // Configure GPIO: PC7 (LED), PC13 (button) for NUCLEO-U585AI-Q
    let led = Output::new(p.PC7, Level::Low, Speed::Low);
    let button = Input::new(p.PC13, Pull::Down);

    defmt::info!("STM32: Spawning actor system (single-core)");
    spawner
        .spawn(control_task_stm32(
            &BUTTON_TO_CONTROL,
            &MAINTENANCE_TO_CONTROL,
        ))
        .unwrap();
    spawner
        .spawn(maintenance_task_stm32(led, &MAINTENANCE_TO_CONTROL))
        .unwrap();
    spawner
        .spawn(button_task_stm32(button, &BUTTON_TO_CONTROL))
        .unwrap();
}
