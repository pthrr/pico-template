// Real-time control actor - runs on Core 0 at 1kHz
// Triggered by hardware timer interrupt for precise timing
package Control {
    private import ScalarValues::*;
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
                doc /* "Waiting for system ready" */
                // Entry action: reset counters
                entry action initialize {
                    cycle_count := 0;
                    control_value := 0.0;
                    error_flag := false;
                }

            }

            state running {
                doc /*"Normal control loop execution"*/
                // Do action: increment cycle counter
                do action count {
                    cycle_count := cycle_count + 1;
                }
            }

            state processing {
                doc /*"Computing control output"*/
                // Do action: compute control value
                do action compute {
                    // Placeholder for control algorithm
                    // control_value := f(inputs)
                }
            }

            state outputting {
                doc /*"Sending control command"*/
                // Do action: send output
                do action send_output {
                    // Send control_value to actuators
                }
            }

            state error {
                doc /*"Error state, control disabled"*/
                entry action set_safe_state {
                    control_value := 0.0;
                    enabled := false;
                }
            }


            // Transitions with guards
            doc /* "System initialization complete after 100 cycles */
            transition init_to_run
                first initializing
                if cycle_count > 100
                then running;

            transition two
                first running
                if enabled
                then processing;



            doc /* Output computed control value */
            transition first processing then outputting;

            doc /* Return to running state */
            transition first outputting then running;

            doc /* Enter error state on fault detection */
            transition first running if error_flag then error;

            doc /* Reset and reinitialize after error */
            transition first error then initializing;

        }



        // Requirements
        requirement {
            subject;
            doc /* Control loop must execute at 1kHz ± 50us jitter */
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
