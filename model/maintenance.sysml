// Maintenance actor - runs on Core 1 at 10Hz
// Toggles LED every 1 second (10 cycles)
package Maintenance {

    // Maintenance actor
    part def MaintenanceActor {
        // Attributes with types and defaults
        attribute tick_count : Integer = 0;
        attribute led_state : Boolean = false;
        attribute system_ok : Boolean = true;
        attribute last_cycle_count : Integer = 0;

        // Timing constraint: 10Hz execution rate (100ms period)
        attribute execution_period_ms : Integer = 100;
        attribute led_toggle_interval_cycles : Integer = 10;  // Toggle every 10 cycles = 1s

        // Performance constraints
        attribute max_execution_time_us : Integer = 5000;  // 5ms max
        attribute priority : Integer = 5;  // Medium priority

        // Core affinity
        attribute core : Integer = 1;  // Core 1 for non-critical tasks

        state machine {
            state idle {
                doc "Waiting for next tick"
                entry action reset_tick {
                    tick_count := 0;
                }
            }

            state checking {
                doc "Checking system health"
                do action increment_tick {
                    tick_count := tick_count + 1;
                }
                do action check_health {
                    // Monitor system status
                    // system_ok := check_system_health()
                }
            }

            state toggling {
                doc "Toggling LED if needed"
                do action toggle_led {
                    led_state := not led_state;
                    tick_count := 0;  // Reset counter after toggle
                }
            }

            state reporting {
                doc "Sending status update"
                do action report_status {
                    // Send system_ok and led_state to other actors
                }
            }

            // Transitions with guards
            transition idle to checking
                doc "Begin health check cycle";

            transition checking to toggling
                when tick_count >= led_toggle_interval_cycles
                doc "Toggle LED after 10 cycles (1 second)";

            transition checking to reporting
                doc "Continue to reporting if no toggle needed";

            transition toggling to reporting
                doc "Report status after toggling LED";

            transition reporting to idle
                doc "Return to idle state";
        }

        // Requirements
        require {
            doc "Maintenance task must execute at 10Hz (100ms period)"
            execution_period_ms == 100
        }

        require {
            doc "LED must toggle every 1 second (10 cycles at 10Hz)"
            led_toggle_interval_cycles == 10
        }

        require {
            doc "Maximum execution time must not exceed 5ms"
            max_execution_time_us <= 5000
        }
    }
}
