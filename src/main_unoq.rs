//! Arduino UNO Q-specific main entry point
//! LED 3 Green is on PH11 (GPIO_ACTIVE_LOW)

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::{Duration, Timer};

#[cfg(feature = "display")]
use embassy_stm32::spi;
#[cfg(feature = "display")]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
#[cfg(feature = "display")]
use embassy_sync::channel::Channel;
#[cfg(feature = "display")]
use pico_template::messages::DisplayState;
#[cfg(feature = "display")]
use static_cell::StaticCell;

#[cfg(feature = "display")]
pico_template::define_channels! {
    CONTROL_TO_DISPLAY: DisplayState, 2;
}

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

#[cfg(feature = "display")]
type Stm32SpiDev = embedded_hal_bus::spi::ExclusiveDevice<
    embassy_stm32::spi::Spi<
        'static,
        embassy_stm32::mode::Blocking,
        embassy_stm32::spi::mode::Master,
    >,
    Output<'static>,
    embedded_hal_bus::spi::NoDelay,
>;

#[cfg(feature = "display")]
#[embassy_executor::task]
async fn display_task_unoq(
    display: &'static mut pico_template::display::driver::DisplayDriver<
        Stm32SpiDev,
        Output<'static>,
    >,
    from_control: &'static Channel<CriticalSectionRawMutex, DisplayState, 2>,
) {
    pico_template::tasks::display_task(display, from_control).await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("Arduino UNO Q: Initializing hardware");
    let p = embassy_stm32::init(Default::default());

    // Initialize heap allocator (required for display feature)
    #[cfg(feature = "display")]
    pico_template::allocator::init_heap();

    // Configure GPIO: PH11 (LED 3 Green) - Active LOW
    // Start HIGH (LED off)
    let led = Output::new(p.PH11, Level::High, Speed::Low);

    defmt::info!("UNO Q: Starting LED blink");
    spawner.spawn(blink_task(led)).expect("spawn blink");

    // Initialize and spawn display task
    #[cfg(feature = "display")]
    {
        use embedded_hal_bus::spi::ExclusiveDevice;
        use pico_template::config::*;
        use pico_template::display::driver::{DisplayDriver, SPI_BUFFER_SIZE};

        // SPI1: SCK=PA5, MOSI=PA7, MISO=PA6 (not used for display)
        let mut spi_config = spi::Config::default();
        spi_config.frequency = embassy_stm32::time::Hertz(DISPLAY_SPI_FREQ_HZ);
        let spi_bus = spi::Spi::new_blocking_txonly(p.SPI1, p.PA5, p.PA7, spi_config);

        // CS pin managed by ExclusiveDevice
        let cs = Output::new(p.PA6, Level::High, Speed::High);
        let spi_dev = match ExclusiveDevice::new_no_delay(spi_bus, cs) {
            Ok(dev) => dev,
            Err(e) => match e {},
        };

        let dc = Output::new(p.PA4, Level::Low, Speed::High);
        let rst = Output::new(p.PA3, Level::Low, Speed::High);

        // Static buffer for SPI interface
        static SPI_BUF: StaticCell<[u8; SPI_BUFFER_SIZE]> = StaticCell::new();
        let buffer = SPI_BUF.init([0u8; SPI_BUFFER_SIZE]);

        let mut delay = embassy_time::Delay;

        #[allow(clippy::cast_possible_truncation)]
        let mipidsi_display = pico_template::display::init::init_display_stm32(
            spi_dev,
            dc,
            rst,
            buffer,
            &mut delay,
            DISPLAY_WIDTH as u16,
            DISPLAY_HEIGHT as u16,
        );

        static DISPLAY: StaticCell<DisplayDriver<Stm32SpiDev, Output<'static>>> = StaticCell::new();
        let display_driver = DISPLAY.init(DisplayDriver::new(mipidsi_display));

        defmt::info!("UNO Q: Spawning display task");
        spawner
            .spawn(display_task_unoq(display_driver, &CONTROL_TO_DISPLAY))
            .expect("spawn display");
    }
}
