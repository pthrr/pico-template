// Button handler actor - runs on Core 1
// Interrupt-driven, responds to button presses
package Button {

    // Button handler actor
    part def ButtonActor {
        // Attributes with types and defaults
        attribute pressed : Boolean = false;
        attribute press_count : Integer = 0;
        attribute debounce_counter : Integer = 0;

        // Timing constraint: Interrupt-driven (asynchronous)
        attribute debounce_threshold : Integer = 5;  // Debounce cycles
        attribute debounce_period_ms : Integer = 10;  // 10ms debounce interval

        // Performance constraints
        attribute max_execution_time_us : Integer = 100;  // Very fast response
        attribute priority : Integer = 8;  // High priority for user interaction

        // Core affinity
        attribute core : Integer = 1;  // Core 1 for I/O handling

        state machine {
            state idle {
                doc "Button not pressed"
                entry action reset_debounce {
                    debounce_counter := 0;
                    pressed := false;
                }
            }

            state debouncing {
                doc "Waiting for stable button state"
                do action increment_debounce {
                    debounce_counter := debounce_counter + 1;
                }
                do action check_stability {
                    // Check if button state is stable
                    // pressed := read_button_state()
                }
            }

            state pressed_state {
                doc "Button confirmed pressed"
                entry action record_press {
                    press_count := press_count + 1;
                }
            }

            state notifying {
                doc "Sending press event"
                do action notify_press {
                    // Send press event to other actors
                    // Notify control system of button press
                }
            }

            state released {
                doc "Button released"
                do action notify_release {
                    // Send release event to other actors
                }
            }

            // Transitions with guards
            transition idle to debouncing
                when pressed
                doc "Begin debouncing on button press detected";

            transition debouncing to pressed_state
                when debounce_counter >= debounce_threshold
                doc "Button press confirmed after stable debounce period";

            transition pressed_state to notifying
                doc "Notify system of confirmed button press";

            transition notifying to released
                doc "Wait for button release";

            transition released to idle
                doc "Return to idle state";
        }

        // Requirements
        require {
            doc "Button debounce must wait for stable state (5 cycles minimum)"
            debounce_threshold >= 5
        }

        require {
            doc "Debounce period must be at least 10ms"
            debounce_period_ms >= 10
        }

        require {
            doc "Button response time must be under 100us"
            max_execution_time_us <= 100
        }
    }
}
