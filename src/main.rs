#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use embassy_executor::{Executor, Spawner};
use embassy_rp::multicore::{Stack, spawn_core1};
use rp2040_boot2 as _;
use {defmt_rtt as _, panic_probe as _};

use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use pico_template::button_hw::ButtonActorHw;
use pico_template::config::*;
use pico_template::control_hw::ControlActorHw;
use pico_template::maintenance_hw::MaintenanceActorHw;
use pico_template::messages::{ButtonMessage, MaintenanceMessage};
use static_cell::StaticCell;

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

// Channels for inter-actor communication
static BUTTON_TO_CONTROL: Channel<CriticalSectionRawMutex, ButtonMessage, 4> = Channel::new();
static MAINTENANCE_TO_CONTROL: Channel<CriticalSectionRawMutex, MaintenanceMessage, 2> =
    Channel::new();

/// Control task (1kHz on Core 0)
#[embassy_executor::task]
async fn control_task() {
    defmt::info!("Control actor starting on Core 0 - target 1kHz (1ms period)");

    let mut actor = ControlActorHw::new(&BUTTON_TO_CONTROL, &MAINTENANCE_TO_CONTROL);

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

/// Maintenance task (10Hz on Core 1, toggles LED every 1s)
#[embassy_executor::task]
async fn maintenance_task(led: Output<'static>) {
    defmt::info!("Maintenance actor starting on Core 1 - 10Hz with 1s LED toggle");

    let mut actor = MaintenanceActorHw::new(led, &MAINTENANCE_TO_CONTROL);

    loop {
        actor.step();

        // 10Hz = 100ms period
        Timer::after(Duration::from_millis(MAINTENANCE_PERIOD_MS as u64)).await;
    }
}

/// Button task (interrupt-driven on Core 1)
#[embassy_executor::task]
async fn button_task(button_pin: Input<'static>) {
    defmt::info!("Button actor starting on Core 1 - interrupt-driven with debouncing");

    let mut actor = ButtonActorHw::new(button_pin, &BUTTON_TO_CONTROL);

    loop {
        Timer::after(Duration::from_millis(BUTTON_DEBOUNCE_MS as u64)).await;
        actor.step();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("Starting multi-core system..");
    let p = embassy_rp::init(Default::default());

    let led = Output::new(p.PIN_25, Level::Low);
    let button = Input::new(p.PIN_2, Pull::Up);

    // Core 0: High-priority real-time control task (1kHz)
    defmt::info!("Core 0: Spawning control task");
    spawner.spawn(control_task()).unwrap();

    // Core 1: Maintenance and button tasks
    defmt::info!("Core 1: Starting executor");
    spawn_core1(p.CORE1, unsafe { &mut CORE1_STACK }, move || {
        let executor = EXECUTOR1.init(Executor::new());
        executor.run(|spawner| {
            defmt::info!("Core 1: Spawning maintenance and button tasks");
            spawner.spawn(maintenance_task(led)).unwrap();
            spawner.spawn(button_task(button)).unwrap();
        });
    });

    defmt::info!("All tasks spawned");
}
