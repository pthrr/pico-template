#![no_std]
#![no_main]

// Platform-specific boot loader (RP2040 only; RP2350 uses internal boot ROM)
#[cfg(feature = "pico1")]
use rp2040_boot2 as _;

// Common panic and debug support
use {defmt_rtt as _, panic_probe as _};

// Platform-specific entry (each feature set has its own `main` via embassy)
#[cfg(any(feature = "pico1", feature = "pico2"))]
mod main_rp2040;

#[cfg(feature = "unoq")]
mod main_unoq;
