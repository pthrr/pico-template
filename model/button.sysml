// Button handler actor - runs on Core 1
// Interrupt-driven, responds to button presses
package Button {
    private import ScalarValues::*;

    action def reset_debounce {
        debounce_counter := 0;
        pressed := false;
    }

    action def increment_debounce {
        debounce_counter := debounce_counter + 1;
    }

    action def record_press {
        press_count := press_count + 1;
    }

    state def Idle {
        doc /*"Button not pressed"*/
        entry action : reset_debounce;
    }

    state def Debouncing {
        doc /*"Waiting for stable button state"*/
        do action : increment_debounce;
    }

    state def PressedState {
        doc /*"Button confirmed pressed"*/
        entry action : record_press;
    }

    state def Notifying {
        doc /*"Sending press event"*/
    }

    state def Released {
        doc /*"Button released"*/
    }

    // Button handler actor
    part def ButtonActor {
        attribute pressed : Boolean = false;
        attribute press_count : Integer = 0;
        attribute debounce_counter : Integer = 0;

        attribute debounce_threshold : Integer = 5;
        attribute debounce_period_ms : Integer = 10;
        attribute max_execution_time_us : Integer = 100;
        attribute priority : Integer = 8;
        attribute core : Integer = 1;

        exhibit state machine {
            state : Idle;
            state : Debouncing;
            state : PressedState;
            state : Notifying;
            state : Released;

            doc /* Begin debouncing on button press detected */
            transition first Idle accept pressed then Debouncing;

            doc /* Button press confirmed after stable debounce period */
            transition first Debouncing if debounce_counter >= debounce_threshold then PressedState;

            doc /* Notify system of confirmed button press */
            transition first PressedState then Notifying;

            doc /* Wait for button release */
            transition first Notifying then Released;

            doc /* Return to idle state */
            transition first Released then Idle;
        }

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
