#![no_std]

#[cfg(all(feature = "pico1", feature = "pico2"))]
compile_error!("Features `pico1` and `pico2` are mutually exclusive. Enable only one.");

#[cfg(all(any(feature = "pico1", feature = "pico2"), feature = "unoq"))]
compile_error!("Platform features `pico1`/`pico2` and `unoq` are mutually exclusive.");

#[cfg(feature = "display")]
extern crate alloc;

#[macro_use]
pub mod actor_channels;
#[cfg(feature = "display")]
pub mod allocator;
#[cfg(feature = "bootloader")]
pub mod bootloader;
#[cfg(feature = "bootloader")]
pub mod bootloader_hw;
pub mod button_hw;
pub mod config;
pub mod control_hw;
#[cfg(feature = "display")]
pub mod display;
#[cfg(feature = "display")]
pub mod display_hw;
#[allow(clippy::all, clippy::pedantic)]
pub mod generated;
#[cfg(feature = "display")]
pub mod gui;
pub mod hal;
pub mod maintenance_hw;
pub mod messages;
pub mod tasks;
