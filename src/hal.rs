//! Hardware Abstraction Layer - Platform-agnostic GPIO traits

/// Trait for digital output pins
pub trait OutputPin {
    fn set_high(&mut self);
    fn set_low(&mut self);
}

/// Trait for digital input pins
pub trait InputPin {
    fn is_low(&self) -> bool;
    fn is_high(&self) -> bool {
        !self.is_low()
    }
}

// RP2040 implementations
#[cfg(any(feature = "pico1", feature = "pico2"))]
impl<'d> OutputPin for embassy_rp::gpio::Output<'d> {
    fn set_high(&mut self) {
        embassy_rp::gpio::Output::set_high(self);
    }
    fn set_low(&mut self) {
        embassy_rp::gpio::Output::set_low(self);
    }
}

#[cfg(any(feature = "pico1", feature = "pico2"))]
impl<'d> InputPin for embassy_rp::gpio::Input<'d> {
    fn is_low(&self) -> bool {
        embassy_rp::gpio::Input::is_low(self)
    }
}

// STM32 implementations
#[cfg(feature = "stm32u585")]
impl<P: embassy_stm32::gpio::Pin> OutputPin for embassy_stm32::gpio::Output<'_, P> {
    fn set_high(&mut self) {
        embassy_stm32::gpio::Output::set_high(self);
    }
    fn set_low(&mut self) {
        embassy_stm32::gpio::Output::set_low(self);
    }
}

#[cfg(feature = "stm32u585")]
impl<P: embassy_stm32::gpio::Pin> InputPin for embassy_stm32::gpio::Input<'_, P> {
    fn is_low(&self) -> bool {
        embassy_stm32::gpio::Input::is_low(self)
    }
}
