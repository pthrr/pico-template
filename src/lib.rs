#![no_std]

#[macro_use]
pub mod actor_channels;
pub mod button_hw;
pub mod config;
pub mod control_hw;
#[allow(clippy::all)]
pub mod generated;
pub mod hal;
pub mod maintenance_hw;
pub mod messages;
pub mod tasks;
