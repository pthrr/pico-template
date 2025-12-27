#![no_std]
#![no_main]
#![allow(static_mut_refs)]

// Platform-specific boot loader
#[cfg(any(feature = "pico1", feature = "pico2"))]
use rp2040_boot2 as _;

// Common panic and debug support
use {defmt_rtt as _, panic_probe as _};

// Include platform-specific main (each platform independent)
#[cfg(any(feature = "pico1", feature = "pico2"))]
#[path = "main_rp2040.rs"]
pub mod main_rp2040;

#[cfg(feature = "stm32u585")]
#[path = "main_stm32.rs"]
pub mod main_stm32;
