//! Host-side behavioral tests for generated Rust (path from `GENERATED_DIR` env).

mod generated {
    pub mod button {
        include!(concat!(env!("GENERATED_DIR"), "/button.rs"));
    }
    pub mod control {
        include!(concat!(env!("GENERATED_DIR"), "/control.rs"));
    }
    pub mod maintenance {
        include!(concat!(env!("GENERATED_DIR"), "/maintenance.rs"));
    }
}

mod button_tests {
    use crate::generated::button::*;

    #[test]
    fn init_is_idle() {
        let a = ButtonActor::new();
        assert_eq!(a.state, ButtonActorState::Idle);
        assert_eq!(a.press_count, 0);
        assert!(!a.pressed);
    }

    #[test]
    fn stays_idle_until_pressed() {
        let mut a = ButtonActor::new();
        for _ in 0..10 {
            a.step();
            assert_eq!(a.state, ButtonActorState::Idle);
        }
    }

    #[test]
    fn debounce_then_press_cycle() {
        let mut a = ButtonActor::new();
        a.pressed = true;

        a.step();
        assert_eq!(a.state, ButtonActorState::Debouncing);

        let threshold = a.debounce_threshold;
        for _ in 0..threshold as usize {
            a.step();
        }
        assert_eq!(a.state, ButtonActorState::PressedState);
        assert_eq!(a.press_count, 1);

        a.step();
        assert_eq!(a.state, ButtonActorState::Notifying);
        a.step();
        assert_eq!(a.state, ButtonActorState::Released);
        a.step();
        assert_eq!(a.state, ButtonActorState::Idle);
        assert!(!a.pressed);
        assert_eq!(a.debounce_counter, 0);
    }

    #[test]
    fn press_count_wraps_at_max() {
        let mut a = ButtonActor::new();
        a.max_press_count = 3;
        a.press_count = 3;
        a.state = ButtonActorState::Debouncing;
        a.debounce_counter = a.debounce_threshold;
        a.step();
        assert_eq!(a.press_count, 0);
    }

    #[test]
    fn timing_constants_match_sysml() {
        assert_eq!(ButtonActor::DEBOUNCE_PERIOD_US, 10_000);
        assert_eq!(ButtonActor::MAX_EXECUTION_TIME_US, 100);
        assert_eq!(ButtonActor::PRIORITY, 8);
        assert_eq!(ButtonActor::CORE, 1);
    }

    #[test]
    fn wcet_lookup_covers_every_state() {
        for st in [
            ButtonActorState::Idle,
            ButtonActorState::Debouncing,
            ButtonActorState::PressedState,
            ButtonActorState::Notifying,
            ButtonActorState::Released,
        ] {
            let _ = ButtonActor::wcet_for_state(st);
        }
        assert!(ButtonActor::WCET_MAX_US > 0);
    }
}

mod maintenance_tests {
    use crate::generated::maintenance::*;

    #[test]
    fn init_is_idle_and_healthy() {
        let a = MaintenanceActor::new();
        assert_eq!(a.state, MaintenanceActorState::Idle);
        assert!(a.system_ok);
        assert!(!a.led_state);
        assert_eq!(a.tick_count, 0);
    }

    #[test]
    fn one_full_loop_increments_tick() {
        let mut a = MaintenanceActor::new();
        a.step();
        assert_eq!(a.state, MaintenanceActorState::Checking);
        a.step();
        assert_eq!(a.state, MaintenanceActorState::Reporting);
        assert_eq!(a.tick_count, 1);
        a.step();
        assert_eq!(a.state, MaintenanceActorState::Idle);
    }

    #[test]
    fn led_toggles_at_threshold() {
        let mut a = MaintenanceActor::new();
        let target = a.led_toggle_interval_cycles;
        for _ in 0..target {
            while a.state != MaintenanceActorState::Idle || a.tick_count == 0 {
                a.step();
                if a.state == MaintenanceActorState::Toggling {
                    break;
                }
                if a.state == MaintenanceActorState::Idle && a.tick_count == 0 {
                    break;
                }
            }
            if a.state == MaintenanceActorState::Toggling {
                break;
            }
            a.step();
        }
        if a.state == MaintenanceActorState::Toggling {
            a.step();
        }
        assert!(a.led_state);
        assert_eq!(a.tick_count, 0);
    }

    #[test]
    fn timing_constants_match_sysml() {
        assert_eq!(MaintenanceActor::EXECUTION_PERIOD_US, 100_000);
        assert_eq!(MaintenanceActor::MAX_EXECUTION_TIME_US, 5_000);
        assert_eq!(MaintenanceActor::PRIORITY, 5);
        assert_eq!(MaintenanceActor::CORE, 1);
    }
}

mod control_tests {
    use crate::generated::control::*;

    #[test]
    fn init_in_initializing_with_clean_outputs() {
        let a = RealtimeControlActor::new();
        assert_eq!(a.state, RealtimeControlActorState::Initializing);
        assert_eq!(a.cycle_count, 0);
        assert_eq!(a.control_value, 0.0);
        assert!(a.enabled);
        assert!(!a.error_flag);
    }

    #[test]
    fn warmup_then_run() {
        let mut a = RealtimeControlActor::new();
        for _ in 0..101 {
            a.step();
        }
        assert_eq!(a.state, RealtimeControlActorState::Running);
    }

    #[test]
    fn error_flag_triggers_safe_state() {
        let mut a = RealtimeControlActor::new();
        a.state = RealtimeControlActorState::Running;
        a.cycle_count = 5;
        a.error_flag = true;
        a.control_value = 42.0;
        a.enabled = true;
        a.step();
        assert_eq!(a.state, RealtimeControlActorState::Error);
        assert_eq!(a.control_value, 0.0);
        assert!(!a.enabled);
    }

    #[test]
    fn watchdog_resets_initializing() {
        let mut a = RealtimeControlActor::new();
        a.state = RealtimeControlActorState::Running;
        a.enabled = true;
        a.error_flag = false;
        a.cycle_count = a.watchdog_limit + 1;
        a.step();
        assert_eq!(a.state, RealtimeControlActorState::Initializing);
        assert_eq!(a.cycle_count, 0);
        assert_eq!(a.control_value, 0.0);
    }

    #[test]
    fn timing_constants_match_sysml() {
        assert_eq!(RealtimeControlActor::EXECUTION_PERIOD_US, 1_000);
        assert_eq!(RealtimeControlActor::MAX_JITTER_US, 50);
        assert_eq!(RealtimeControlActor::MAX_EXECUTION_TIME_US, 800);
        assert_eq!(RealtimeControlActor::PRIORITY, 10);
        assert_eq!(RealtimeControlActor::CORE, 0);
        assert!(RealtimeControlActor::WCET_MAX_US <= 1_000);
    }
}
