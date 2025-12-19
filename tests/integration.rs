#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use pico_template::generated::button::ButtonActor;
use pico_template::generated::control::RealtimeControlActor;
use pico_template::generated::maintenance::MaintenanceActor;

#[defmt_test::tests]
mod tests {
    use super::*;

    #[init]
    fn init() -> () {
        defmt::info!("Initializing test environment");
    }

    #[test]
    fn test_button_actor_init(#[allow(unused)] _state: &mut ()) {
        defmt::info!("Testing ButtonActor initialization");
        let actor = ButtonActor::new();
        assert!(actor.press_count == 0);
        assert!(actor.pressed == false);
        defmt::info!("ButtonActor initialization test passed");
    }

    #[test]
    fn test_maintenance_actor_init(#[allow(unused)] _state: &mut ()) {
        defmt::info!("Testing MaintenanceActor initialization");
        let actor = MaintenanceActor::new();
        assert!(actor.tick_count == 0);
        assert!(actor.led_state == false);
        assert!(actor.system_ok == true);
        defmt::info!("MaintenanceActor initialization test passed");
    }

    #[test]
    fn test_control_actor_init(#[allow(unused)] _state: &mut ()) {
        defmt::info!("Testing RealtimeControlActor initialization");
        let actor = RealtimeControlActor::new();
        assert!(actor.cycle_count == 0);
        assert!(actor.enabled == true);
        assert!(actor.error_flag == false);
        defmt::info!("RealtimeControlActor initialization test passed");
    }

    #[test]
    fn test_button_actor_step(#[allow(unused)] _state: &mut ()) {
        defmt::info!("Testing ButtonActor state machine step");
        let mut actor = ButtonActor::new();

        // Initial state should be Idle
        assert!(matches!(
            actor.state,
            pico_template::generated::button::ButtonActorState::Idle
        ));

        // Step with no press should stay in Idle
        actor.step();
        assert!(matches!(
            actor.state,
            pico_template::generated::button::ButtonActorState::Idle
        ));

        defmt::info!("ButtonActor step test passed");
    }

    #[test]
    fn test_maintenance_actor_step(#[allow(unused)] _state: &mut ()) {
        defmt::info!("Testing MaintenanceActor state machine step");
        let mut actor = MaintenanceActor::new();

        // Initial state should be Idle
        assert!(matches!(
            actor.state,
            pico_template::generated::maintenance::MaintenanceActorState::Idle
        ));

        // Step should transition to Checking
        actor.step();

        defmt::info!("MaintenanceActor step test passed");
    }
}
