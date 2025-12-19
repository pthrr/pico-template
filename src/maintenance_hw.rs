//! Maintenance actor implementation with hardware integration

use crate::generated::maintenance::{MaintenanceActor, MaintenanceActorState};
use crate::messages::MaintenanceMessage;
use embassy_rp::gpio::Output;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

/// Specialized maintenance actor with hardware resources
pub struct MaintenanceActorHw {
    pub actor: MaintenanceActor,
    pub led: Output<'static>,
    pub to_control: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
}

impl MaintenanceActorHw {
    pub fn new(
        led: Output<'static>,
        to_control: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
    ) -> Self {
        Self {
            actor: MaintenanceActor::new(),
            led,
            to_control,
        }
    }

    pub fn step(&mut self) {
        // Execute state machine
        let old_state = self.actor.state;
        self.actor.step();

        // Handle state transitions - produce outputs
        if old_state != self.actor.state {
            match self.actor.state {
                MaintenanceActorState::Toggling => {
                    // Update hardware based on state machine output
                    if self.actor.led_state {
                        self.led.set_high();
                        defmt::info!("Maintenance: LED ON");
                    } else {
                        self.led.set_low();
                        defmt::info!("Maintenance: LED OFF");
                    }
                }
                MaintenanceActorState::Reporting => {
                    // Send message
                    let msg = MaintenanceMessage {
                        system_ok: self.actor.system_ok,
                        led_state: self.actor.led_state,
                        tick_count: self.actor.tick_count,
                    };
                    defmt::debug!(
                        "Maintenance: Reporting status (ok={}, led={}, tick={})",
                        msg.system_ok,
                        msg.led_state,
                        msg.tick_count
                    );
                    let _ = self.to_control.try_send(msg);
                }
                _ => {}
            }
        }
    }
}
