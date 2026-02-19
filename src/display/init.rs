//! Platform-specific display initialization
//!
//! Configures the mipidsi Builder with correct orientation, size,
//! and color inversion for each supported display controller.
//!
//! Reset is performed manually before calling Builder::init, so
//! the Display type uses `NoResetPin` for the RST parameter.

use mipidsi::NoResetPin;
use mipidsi::interface::SpiInterface;
use mipidsi::options::{ColorInversion, Orientation, Rotation};

use super::driver::LcdModel;

/// Initialize display for any platform.
///
/// Performs a manual hardware reset via `rst`, then initializes the
/// display using `NoResetPin` (so the Display type doesn't carry RST).
///
/// Panics on init failure (hardware fault).
pub fn init_display<SPI, DC, RST, DELAY>(
    spi: SPI,
    dc: DC,
    mut rst: RST,
    buffer: &'static mut [u8],
    delay: &mut DELAY,
    width: u16,
    height: u16,
) -> mipidsi::Display<SpiInterface<'static, SPI, DC>, LcdModel, NoResetPin>
where
    SPI: embedded_hal::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin,
    RST: embedded_hal::digital::OutputPin,
    DELAY: embedded_hal::delay::DelayNs,
{
    // Manual hardware reset
    let _ = rst.set_low();
    delay.delay_us(10);
    let _ = rst.set_high();
    delay.delay_ms(120);

    let di = SpiInterface::new(spi, dc, buffer);

    let display = mipidsi::Builder::new(lcd_model(), di)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .invert_colors(color_inversion())
        .display_size(width, height)
        .init(delay);

    match display {
        Ok(d) => {
            defmt::info!("Display initialized: {}x{}", width, height);
            d
        }
        Err(_) => {
            defmt::panic!("Display init failed");
        }
    }
}

/// Construct the appropriate LCD model based on the selected display feature.
fn lcd_model() -> LcdModel {
    #[cfg(feature = "display-st7789")]
    {
        mipidsi::models::ST7789
    }
    #[cfg(feature = "display-ili9341")]
    {
        mipidsi::models::ILI9341Rgb565
    }
}

/// Color inversion setting per display controller.
/// ST7789 typically needs inversion enabled; ILI9341 does not.
const fn color_inversion() -> ColorInversion {
    #[cfg(feature = "display-st7789")]
    {
        ColorInversion::Inverted
    }
    #[cfg(feature = "display-ili9341")]
    {
        ColorInversion::Normal
    }
}
