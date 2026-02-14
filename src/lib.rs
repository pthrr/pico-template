#![no_std]

#[cfg(feature = "display")]
extern crate alloc;

#[macro_use]
pub mod actor_channels;
#[cfg(feature = "display")]
pub mod allocator;
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
