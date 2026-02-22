// Maintenance actor - runs on Core 1 at 10Hz
// Toggles LED every 1 second (10 cycles)
package Maintenance {
    private import ScalarValues::*;

    port def StatusPort {
        out signal system_ok : Boolean;
        out signal led_state : Boolean;
        out signal tick_count : Integer;
    }

    action def increment_tick {
        tick_count := tick_count + 1;
    }

    action def toggle_led {
        led_state := not led_state;
        tick_count := 0;
    }

    state def Idle {
        doc /*"Waiting for next tick"*/
    }

    state def Checking {
        doc /*"Checking system health"*/
        do action : increment_tick;
    }

    state def Toggling {
        doc /*"Toggling LED if needed"*/
        do action : toggle_led;
    }

    state def Reporting {
        doc /*"Sending status update"*/
    }

    // Maintenance actor
    part def MaintenanceActor {
        port status_out : StatusPort;

        attribute tick_count : Integer = 0;
        attribute led_state : Boolean = false;
        attribute system_ok : Boolean = true;
        attribute last_cycle_count : Integer = 0;

        attribute execution_period_ms : Integer = 100;
        attribute led_toggle_interval_cycles : Integer = 10;
        attribute max_execution_time_us : Integer = 5000;
        attribute priority : Integer = 5;
        attribute core : Integer = 1;

        exhibit state machine {
            state : Idle;
            state : Checking;
            state : Toggling;
            state : Reporting;

            doc /* Begin health check cycle */
            transition first Idle then Checking;

            doc /* Toggle LED after 10 cycles (1 second) */
            transition first Checking if tick_count >= led_toggle_interval_cycles then Toggling;

            doc /* Continue to reporting if no toggle needed */
            transition first Checking then Reporting;

            doc /* Report status after toggling LED */
            transition first Toggling then Reporting;

            doc /* Return to idle state */
            transition first Reporting then Idle;
        }

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
