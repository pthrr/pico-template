// Maintenance actor - runs on Core 1 at 10Hz
// Toggles LED every 1 second (10 cycles)
package Maintenance {
    private import ScalarValues::*;

    // Maintenance actor
    part def MaintenanceActor {
        // Attributes with types and defaults
        attribute tick_count : Integer = 0;
        attribute led_state : Boolean = false;
        attribute system_ok : Boolean = true;
        attribute last_cycle_count : Integer = 0;

        // Timing constraint: 10Hz execution rate (100ms period)
        attribute execution_period_ms : Integer = 100;
        attribute led_toggle_interval_cycles : Integer = 10;

        // Performance constraints
        attribute max_execution_time_us : Integer = 5000;
        attribute priority : Integer = 5;

        // Core affinity
        attribute core : Integer = 1;

        state machine {
            state idle {
                doc /*"Waiting for next tick"*/
                entry action reset_tick {
                    tick_count := 0;
                }
            }

            state checking {
                doc /*"Checking system health"*/
                do action increment_tick {
                    tick_count := tick_count + 1;
                }
                do action check_health {
                    // Monitor system status
                    // system_ok := check_system_health()
                }
            }

            state toggling {
                doc /*"Toggling LED if needed"*/
                do action toggle_led {
                    led_state := not led_state;
                    tick_count := 0;
                }
            }

            state reporting {
                doc /*"Sending status update"*/
                do action report_status {
                    // Send system_ok and led_state to other actors
                }
            }

            doc /* Begin health check cycle */
            transition first idle then checking;

            doc /* Toggle LED after 10 cycles (1 second) */
            transition first checking if tick_count >= led_toggle_interval_cycles then toggling;

            doc /* Continue to reporting if no toggle needed */
            transition first checking then reporting;

            doc /* Report status after toggling LED */
            transition first toggling then reporting;

            doc /* Return to idle state */
            transition first reporting then idle;
        }

        // Requirements
        requirement {
            subject;
            doc /* Maintenance task must execute at 10Hz (100ms period) */
            require constraint {
                execution_period_ms == 100
            }
        }

        requirement {
            subject;
            doc /* LED must toggle every 1 second (10 cycles at 10Hz) */
            require constraint {
                led_toggle_interval_cycles == 10
            }
        }

        requirement {
            subject;
            doc /* Maximum execution time must not exceed 5ms */
            require constraint {
                max_execution_time_us <= 5000
            }
        }
    }
}
