//! LCD display type wrapper
//!
//! Provides a unified `LcdDisplay` type alias that wraps mipidsi's `Display`
//! with feature-gated model selection (ST7789 or ILI9341).

#[cfg(all(feature = "display-st7789", feature = "display-ili9341"))]
compile_error!(
    "Features `display-st7789` and `display-ili9341` are mutually exclusive. Enable only one."
);

#[cfg(not(any(feature = "display-st7789", feature = "display-ili9341")))]
compile_error!(
    "Feature `display` requires either `display-st7789` or `display-ili9341` to be enabled."
);

use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use mipidsi::NoResetPin;
use mipidsi::interface::SpiInterface;

/// ST7789 LCD display model (240x320 typical)
#[cfg(feature = "display-st7789")]
pub type LcdModel = mipidsi::models::ST7789;

/// ILI9341 LCD display model (240x320 typical)
#[cfg(feature = "display-ili9341")]
pub type LcdModel = mipidsi::models::ILI9341Rgb565;

/// SPI transfer buffer size for mipidsi.
/// Larger buffers improve throughput at the cost of RAM.
pub const SPI_BUFFER_SIZE: usize = 512;

/// Wrapper that owns the mipidsi Display and delegates DrawTarget.
///
/// The reset pin is consumed during init, so the Display stores `NoResetPin`
/// (we toggle reset manually before calling Builder::init).
pub struct DisplayDriver<SPI, DC>
where
    SPI: embedded_hal::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin,
{
    display: mipidsi::Display<SpiInterface<'static, SPI, DC>, LcdModel, NoResetPin>,
}

impl<SPI, DC> DisplayDriver<SPI, DC>
where
    SPI: embedded_hal::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin,
{
    /// Create a new display driver from an already-initialized mipidsi Display.
    pub fn new(
        display: mipidsi::Display<SpiInterface<'static, SPI, DC>, LcdModel, NoResetPin>,
    ) -> Self {
        Self { display }
    }
}

impl<SPI, DC> DrawTarget for DisplayDriver<SPI, DC>
where
    SPI: embedded_hal::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin,
{
    type Color = Rgb565;
    type Error =
        <mipidsi::Display<SpiInterface<'static, SPI, DC>, LcdModel, NoResetPin> as DrawTarget>::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.display.draw_iter(pixels)
    }
}

impl<SPI, DC> OriginDimensions for DisplayDriver<SPI, DC>
where
    SPI: embedded_hal::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin,
{
    fn size(&self) -> Size {
        self.display.size()
    }
}
