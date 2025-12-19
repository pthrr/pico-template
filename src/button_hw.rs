//! Button actor implementation with hardware integration

use crate::generated::button::{ButtonActor, ButtonActorState};
use crate::messages::ButtonMessage;
use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

/// Specialized button actor with hardware resources
pub struct ButtonActorHw {
    pub actor: ButtonActor,
    pub button_pin: Input<'static>,
    pub to_control: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
}

impl ButtonActorHw {
    pub fn new(
        button_pin: Input<'static>,
        to_control: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
    ) -> Self {
        Self {
            actor: ButtonActor::new(),
            button_pin,
            to_control,
        }
    }

    pub fn step(&mut self) {
        // Update inputs from hardware
        self.actor.pressed = self.button_pin.is_low();

        // Execute state machine
        let old_state = self.actor.state;
        self.actor.step();

        // Handle state transitions - produce outputs
        if old_state != self.actor.state {
            match self.actor.state {
                ButtonActorState::Notifying => {
                    defmt::info!("Button: Sending press event");
                    let _ = self.to_control.try_send(ButtonMessage::Pressed);
                }
                ButtonActorState::Released => {
                    defmt::info!("Button: Sending release event");
                    let _ = self.to_control.try_send(ButtonMessage::Released);
                }
                _ => {}
            }
        }
    }
}
