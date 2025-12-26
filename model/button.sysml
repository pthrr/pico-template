// Button handler actor - runs on Core 1
// Interrupt-driven, responds to button presses
package Button {
    private import ScalarValues::*;

    // Button handler actor
    part def ButtonActor {
        // Attributes with types and defaults
        attribute pressed : Boolean = false;
        attribute press_count : Integer = 0;
        attribute debounce_counter : Integer = 0;

        // Timing constraint: Interrupt-driven (asynchronous)
        attribute debounce_threshold : Integer = 5;
        attribute debounce_period_ms : Integer = 10;

        // Performance constraints
        attribute max_execution_time_us : Integer = 100;
        attribute priority : Integer = 8;

        // Core affinity
        attribute core : Integer = 1;

        state machine {
            state idle {
                doc /*"Button not pressed"*/
                entry action reset_debounce {
                    debounce_counter := 0;
                    pressed := false;
                }
            }

            state debouncing {
                doc /*"Waiting for stable button state"*/
                do action increment_debounce {
                    debounce_counter := debounce_counter + 1;
                }
                do action check_stability {
                    // Check if button state is stable
                    // pressed := read_button_state()
                }
            }

            state pressed_state {
                doc /*"Button confirmed pressed"*/
                entry action record_press {
                    press_count := press_count + 1;
                }
            }

            state notifying {
                doc /*"Sending press event"*/
                do action notify_press {
                    // Send press event to other actors
                    // Notify control system of button press
                }
            }

            state released {
                doc /*"Button released"*/
                do action notify_release {
                    // Send release event to other actors
                }
            }

            doc /* Begin debouncing on button press detected */
            transition first idle if pressed then debouncing;

            doc /* Button press confirmed after stable debounce period */
            transition first debouncing if debounce_counter >= debounce_threshold then pressed_state;

            doc /* Notify system of confirmed button press */
            transition first pressed_state then notifying;

            doc /* Wait for button release */
            transition first notifying then released;

            doc /* Return to idle state */
            transition first released then idle;
        }

        // Requirements
        requirement {
            subject;
            doc /* Button debounce must wait for stable state (5 cycles minimum) */
            require constraint {
                debounce_threshold >= 5
            }
        }

        requirement {
            subject;
            doc /* Debounce period must be at least 10ms */
            require constraint {
                debounce_period_ms >= 10
            }
        }

        requirement {
            subject;
            doc /* Button response time must be under 100us */
            require constraint {
                max_execution_time_us <= 100
            }
        }
    }
}
