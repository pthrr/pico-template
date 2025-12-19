// Real-time control actor - runs on Core 0 at 1kHz
// Triggered by hardware timer interrupt for precise timing
package Control {

    // Real-time control actor
    part def RealtimeControlActor {
        // Attributes with types and defaults
        attribute control_value : Real = 0.0;
        attribute cycle_count : Integer = 0;
        attribute error_flag : Boolean = false;
        attribute enabled : Boolean = true;

        // Timing constraint: 1kHz execution rate (1ms period)
        attribute execution_period_ms : Integer = 1;
        attribute max_jitter_us : Integer = 50;

        // Performance constraints
        attribute max_execution_time_us : Integer = 800;
        attribute priority : Integer = 10;  // High priority for real-time

        // Core affinity
        attribute core : Integer = 0;  // Core 0 for time-critical tasks

        state machine {
            state initializing {
                doc "Waiting for system ready"
                // Entry action: reset counters
                entry action initialize {
                    cycle_count := 0;
                    control_value := 0.0;
                    error_flag := false;
                }
            }

            state running {
                doc "Normal control loop execution"
                // Do action: increment cycle counter
                do action count {
                    cycle_count := cycle_count + 1;
                }
            }

            state processing {
                doc "Computing control output"
                // Do action: compute control value
                do action compute {
                    // Placeholder for control algorithm
                    // control_value := f(inputs)
                }
            }

            state outputting {
                doc "Sending control command"
                // Do action: send output
                do action send_output {
                    // Send control_value to actuators
                }
            }

            state error {
                doc "Error state, control disabled"
                entry action set_safe_state {
                    control_value := 0.0;
                    enabled := false;
                }
            }

            // Transitions with guards
            transition initializing to running
                when cycle_count > 100
                doc "System initialization complete after 100 cycles";

            transition running to processing
                when enabled
                doc "Begin processing if system is enabled";

            transition processing to outputting
                doc "Output computed control value";

            transition outputting to running
                doc "Return to running state";

            transition running to error
                when error_flag
                doc "Enter error state on fault detection";

            transition error to initializing
                doc "Reset and reinitialize after error";
        }

        // Requirements
        require {
            doc "Control loop must execute at 1kHz ± 50us jitter"
            execution_period_ms == 1 and max_jitter_us <= 50
        }

        require {
            doc "Maximum execution time must not exceed 800us"
            max_execution_time_us <= 800
        }
    }
}
