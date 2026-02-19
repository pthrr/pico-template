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
    #[cfg(feature = "display")]
    pub to_display: &'static Channel<CriticalSectionRawMutex, crate::messages::DisplayState, 2>,
    // Tracked state for display snapshots
    #[cfg(feature = "display")]
    cycle_count: i32,
    #[cfg(feature = "display")]
    last_maintenance_ok: bool,
    #[cfg(feature = "display")]
    last_led_state: bool,
    #[cfg(feature = "display")]
    last_maintenance_tick: i32,
    #[cfg(feature = "display")]
    last_button_pressed: bool,
}

impl ControlActorHw {
    pub fn new(
        from_button: &'static Channel<CriticalSectionRawMutex, ButtonMessage, 4>,
        from_maintenance: &'static Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>,
        #[cfg(feature = "display")] to_display: &'static Channel<
            CriticalSectionRawMutex,
            crate::messages::DisplayState,
            2,
        >,
    ) -> Self {
        Self {
            actor: RealtimeControlActor::new(),
            from_button,
            from_maintenance,
            #[cfg(feature = "display")]
            to_display,
            #[cfg(feature = "display")]
            cycle_count: 0,
            #[cfg(feature = "display")]
            last_maintenance_ok: true,
            #[cfg(feature = "display")]
            last_led_state: false,
            #[cfg(feature = "display")]
            last_maintenance_tick: 0,
            #[cfg(feature = "display")]
            last_button_pressed: false,
        }
    }

    pub fn step(&mut self) {
        // Process incoming messages
        while let Ok(msg) = self.from_button.try_receive() {
            match msg {
                ButtonMessage::Pressed => {
                    #[cfg(feature = "display")]
                    {
                        self.last_button_pressed = true;
                    }
                    defmt::info!("Control: Button pressed");
                }
                ButtonMessage::Released => {
                    #[cfg(feature = "display")]
                    {
                        self.last_button_pressed = false;
                    }
                    defmt::info!("Control: Button released");
                }
            }
        }

        while let Ok(msg) = self.from_maintenance.try_receive() {
            #[cfg(feature = "display")]
            {
                self.last_maintenance_ok = msg.system_ok;
                self.last_led_state = msg.led_state;
                self.last_maintenance_tick = msg.tick_count;
            }
            defmt::debug!(
                "Control: Maintenance status (ok={}, led={}, tick={})",
                msg.system_ok,
                msg.led_state,
                msg.tick_count
            );
        }

        // Execute state machine
        self.actor.step();
        #[cfg(feature = "display")]
        {
            self.cycle_count = self.cycle_count.wrapping_add(1);
        }

        // Emit display state snapshot (non-blocking, latest-wins)
        #[cfg(feature = "display")]
        {
            let uptime_secs = (embassy_time::Instant::now().as_millis() / 1000) as u32;
            let snapshot = crate::messages::DisplayState {
                control_cycle: self.cycle_count,
                maintenance_ok: self.last_maintenance_ok,
                led_state: self.last_led_state,
                maintenance_tick: self.last_maintenance_tick,
                button_pressed: self.last_button_pressed,
                uptime_secs,
            };
            let _ = self.to_display.try_send(snapshot);
        }
    }
}
