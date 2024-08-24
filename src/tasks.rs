//! Thin platform-agnostic task glue.
//!
//! Each `*_task` constructs the actor's hardware adapter and hands a
//! `|| actor.step()` closure to the generated scheduling loop in
//! `crate::generated::tasks` (period, deadline, WCET from `SysML`).

use crate::button_hw::ButtonActorHw;
use crate::control_hw::ControlActorHw;
use crate::generated::tasks as generated_tasks;
use crate::hal::{InputPin, OutputPin};
use crate::maintenance_hw::MaintenanceActorHw;
use crate::messages::{ButtonMessage, MaintenanceMessage};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

/// 1 kHz control actor task.
pub async fn control_task(
    from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    let mut actor = ControlActorHw::new(from_button, from_maintenance);
    generated_tasks::realtime_control_loop(|| actor.step()).await;
}

/// Maintenance actor task.
pub async fn maintenance_task<O: OutputPin>(
    led: O,
    to_control: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
) {
    let mut actor = MaintenanceActorHw::new(led, to_control);
    generated_tasks::maintenance_loop(|| actor.step()).await;
}

/// Button actor task (debounce-polled).
pub async fn button_task<I: InputPin>(
    button_pin: I,
    to_control: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
) {
    let mut actor = ButtonActorHw::new(button_pin, to_control);
    generated_tasks::button_loop(|| actor.step()).await;
}
