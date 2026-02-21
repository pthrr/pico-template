// Real-time control actor - runs on Core 0 at 1kHz
// Triggered by hardware timer interrupt for precise timing
package Control {
    private import ScalarValues::*;

    action def initialize {
        cycle_count := 0;
        control_value := 0.0;
        error_flag := false;
    }

    action def count {
        cycle_count := cycle_count + 1;
    }

    action def set_safe_state {
        control_value := 0.0;
        enabled := false;
    }

    state def Initializing {
        doc /* "Waiting for system ready" */
        entry action : initialize;
        do action : count;
    }

    state def Running {
        doc /*"Normal control loop execution"*/
        do action : count;
    }

    state def Processing {
        doc /*"Computing control output"*/
    }

    state def Outputting {
        doc /*"Sending control command"*/
    }

    state def Error {
        doc /*"Error state, control disabled"*/
        entry action : set_safe_state;
    }

    // Real-time control actor
    part def RealtimeControlActor {
        attribute control_value : Real = 0.0;
        attribute cycle_count : Integer = 0;
        attribute error_flag : Boolean = false;
        attribute enabled : Boolean = true;

        attribute execution_period_ms : Integer = 1;
        attribute max_jitter_us : Integer = 50;
        attribute max_execution_time_us : Integer = 800;
        attribute priority : Integer = 10;
        attribute core : Integer = 0;

        exhibit state machine {
            state : Initializing;
            state : Running;
            state : Processing;
            state : Outputting;
            state : Error;

            doc /* "System initialization complete after 100 cycles */
            transition init_to_run
                first Initializing
                if cycle_count > 100
                then Running;

            doc /* Enter error state on fault detection (highest priority) */
            transition first Running accept error_flag then Error;

            transition two
                first Running
                if enabled
                then Processing;

            doc /* Output computed control value */
            transition first Processing then Outputting;

            doc /* Return to running state */
            transition first Outputting then Running;

            doc /* Reset and reinitialize after error */
            transition first Error then Initializing;
        }

        requirement {
            subject;
            doc /* Control loop must execute at 1kHz +/- 50us jitter */
            require constraint {
                execution_period_ms <= 1 and max_jitter_us <= 50e-6 [SI::s]
            }
        }

        requirement {
            subject;
            doc /* Maximum execution time must not exceed 800us */
            require constraint {
                max_execution_time_us <= 800e-6 [SI::s]
            }
        }
    }
}
