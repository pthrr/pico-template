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

#[cfg(feature = "display")]
use pico_template::messages::DisplayState;

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

// Define channels for inter-actor communication (owned by RP2040 platform)
define_channels! {
    BUTTON_TO_CONTROL: ButtonMessage, 4;
    MAINTENANCE_TO_CONTROL: MaintenanceMessage, 2;
}

#[cfg(feature = "display")]
define_channels! {
    CONTROL_TO_DISPLAY: DisplayState, 2;
}

// RP2040 Embassy task wrappers (concrete types required)
#[embassy_executor::task]
async fn control_task_rp2040(
    from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
    #[cfg(feature = "display")] to_display: &'static Channel<
        CriticalSectionRawMutex,
        DisplayState,
        2,
    >,
) {
    tasks::control_task(
        from_button,
        from_maintenance,
        #[cfg(feature = "display")]
        to_display,
    )
    .await
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

#[cfg(feature = "display")]
type RpSpiDev = embedded_hal_bus::spi::ExclusiveDevice<
    embassy_rp::spi::Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Blocking>,
    Output<'static>,
    embedded_hal_bus::spi::NoDelay,
>;

#[cfg(feature = "display")]
#[embassy_executor::task]
async fn display_task_rp2040(
    display: &'static mut pico_template::display::driver::DisplayDriver<RpSpiDev, Output<'static>>,
    from_control: &'static Channel<CriticalSectionRawMutex, DisplayState, 2>,
) {
    tasks::display_task(display, from_control).await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::info!("RP2040: Initializing hardware");
    let p = embassy_rp::init(Default::default());

    // Initialize heap allocator (required for display feature)
    #[cfg(feature = "display")]
    pico_template::allocator::init_heap();

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
            #[cfg(feature = "display")]
            &CONTROL_TO_DISPLAY,
        ))
        .expect("spawn control");

    // Core 1: Maintenance, button, and optionally display tasks
    defmt::info!("Core 1: Starting secondary executor");

    // Initialize display hardware before moving into Core 1 closure
    #[cfg(feature = "display")]
    let display_driver = {
        use embassy_rp::spi::{self, Spi};
        use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
        use pico_template::display::driver::{DisplayDriver, SPI_BUFFER_SIZE};

        // SPI0 pins for display: SCK=PIN_18, MOSI=PIN_19
        let mut spi_config = spi::Config::default();
        spi_config.frequency = DISPLAY_SPI_FREQ_HZ;
        let spi_bus = Spi::new_blocking_txonly(p.SPI0, p.PIN_18, p.PIN_19, spi_config);

        // CS pin managed by ExclusiveDevice (wraps SpiBus into SpiDevice)
        let cs = Output::new(p.PIN_17, Level::High);
        let spi_dev = match ExclusiveDevice::new_no_delay(spi_bus, cs) {
            Ok(dev) => dev,
            Err(e) => match e {},
        };

        let dc = Output::new(p.PIN_20, Level::Low);
        let rst = Output::new(p.PIN_21, Level::Low);

        // Static buffer for SPI interface
        static SPI_BUF: StaticCell<[u8; SPI_BUFFER_SIZE]> = StaticCell::new();
        let buffer = SPI_BUF.init([0u8; SPI_BUFFER_SIZE]);

        // Blocking delay for display init
        let mut delay = embassy_time::Delay;

        type RpSpiDev = ExclusiveDevice<
            Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Blocking>,
            Output<'static>,
            NoDelay,
        >;

        #[allow(clippy::cast_possible_truncation)]
        let mipidsi_display = pico_template::display::init::init_display_rp(
            spi_dev,
            dc,
            rst,
            buffer,
            &mut delay,
            DISPLAY_WIDTH as u16,
            DISPLAY_HEIGHT as u16,
        );

        static DISPLAY: StaticCell<DisplayDriver<RpSpiDev, Output<'static>>> = StaticCell::new();
        DISPLAY.init(DisplayDriver::new(mipidsi_display))
    };

    spawn_core1(p.CORE1, unsafe { &mut CORE1_STACK }, move || {
        let executor = EXECUTOR1.init(Executor::new());
        executor.run(|spawner| {
            defmt::info!("Core 1: Spawning peripheral tasks");
            spawner
                .spawn(maintenance_task_rp2040(led, &MAINTENANCE_TO_CONTROL))
                .expect("spawn maintenance");
            spawner
                .spawn(button_task_rp2040(button, &BUTTON_TO_CONTROL))
                .expect("spawn button");

            #[cfg(feature = "display")]
            {
                defmt::info!("Core 1: Spawning display task");
                spawner
                    .spawn(display_task_rp2040(display_driver, &CONTROL_TO_DISPLAY))
                    .expect("spawn display");
            }
        });
    });
}
