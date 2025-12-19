//! Control actor implementation with hardware integration

use crate::generated::control::RealtimeControlActor;
use crate::messages::{ButtonMessage, MaintenanceMessage};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

/// Specialized control actor with message channels
pub struct ControlActorHw {
    pub actor: RealtimeControlActor,
    pub from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    pub from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
}

impl ControlActorHw {
    pub fn new(
        from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
        from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
    ) -> Self {
        Self {
            actor: RealtimeControlActor::new(),
            from_button,
            from_maintenance,
        }
    }

    pub fn step(&mut self) {
        // Process incoming messages
        while let Ok(msg) = self.from_button.try_receive() {
            match msg {
                ButtonMessage::Pressed => {
                    defmt::info!("Control: Button pressed");
                }
                ButtonMessage::Released => {
                    defmt::info!("Control: Button released");
                }
            }
        }

        while let Ok(msg) = self.from_maintenance.try_receive() {
            defmt::debug!(
                "Control: Maintenance status (ok={}, led={}, tick={})",
                msg.system_ok,
                msg.led_state,
                msg.tick_count
            );
        }

        // Execute state machine
        self.actor.step();
    }
}
