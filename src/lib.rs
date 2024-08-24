#![no_std]

#[cfg(all(feature = "pico1", feature = "pico2"))]
compile_error!("Features `pico1` and `pico2` are mutually exclusive. Enable only one.");

#[cfg(all(any(feature = "pico1", feature = "pico2"), feature = "unoq"))]
compile_error!("Platform features `pico1`/`pico2` and `unoq` are mutually exclusive.");

pub mod button_hw;
pub mod config;
pub mod control_hw;
pub mod generated;
pub mod hal;
pub mod maintenance_hw;
pub mod messages;
pub mod tasks;
