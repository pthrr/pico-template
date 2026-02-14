//! Arduino UNO Q-specific main entry point
//! LED 3 Green is on PH11 (GPIO_ACTIVE_LOW)

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::{Duration, Timer};

#[embassy_executor::task]
async fn blink_task(mut led: Output<'static>) {
    loop {
        // LED is active-low: LOW = on, HIGH = off
        led.set_low();
        defmt::info!("LED ON");
        Timer::after(Duration::from_millis(500)).await;

        led.set_high();
        defmt::info!("LED OFF");
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("Arduino UNO Q: Initializing hardware");
    let p = embassy_stm32::init(Default::default());

    // Configure GPIO: PH11 (LED 3 Green) - Active LOW
    // Start HIGH (LED off)
    let led = Output::new(p.PH11, Level::High, Speed::Low);

    defmt::info!("UNO Q: Starting LED blink");
    spawner.spawn(blink_task(led)).unwrap();
}
