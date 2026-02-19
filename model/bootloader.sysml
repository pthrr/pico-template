// Bootloader actor - firmware update via USB CDC (RP) or UART (STM32)
// Runs on Core 1 (RP) or main executor (STM32)
package Bootloader {
    private import ScalarValues::*;

    part def BootloaderActor {
        // Flags set by _hw wrapper before calling step()
        attribute active : Boolean = false;
        attribute erase_requested : Boolean = false;
        attribute erase_complete : Boolean = false;
        attribute chunk_ready : Boolean = false;
        attribute chunk_written : Boolean = false;
        attribute commit_requested : Boolean = false;
        attribute verify_ok : Boolean = false;
        attribute error_flag : Boolean = false;

        // Counters / state tracked by actor
        attribute bytes_received : Integer = 0;
        attribute chunks_written : Integer = 0;
        attribute total_size : Integer = 0;
        attribute error_code : Integer = 0;

        // Actor metadata
        attribute core : Integer = 1;
        attribute priority : Integer = 3;

        state machine {
            state idle {
                doc /*"Waiting for bootloader activation"*/
                entry action reset_state {
                    bytes_received := 0;
                    chunks_written := 0;
                    total_size := 0;
                    error_code := 0;
                    error_flag := false;
                }
            }

            state connected {
                doc /*"Connection established, awaiting commands"*/
                entry action clear_requests {
                    erase_requested := false;
                    erase_complete := false;
                    chunk_ready := false;
                    chunk_written := false;
                    commit_requested := false;
                    verify_ok := false;
                }
            }

            state erasing {
                doc /*"Erasing staging flash area"*/
            }

            state receiving {
                doc /*"Writing firmware chunk to staging"*/
                do action count_chunk {
                    chunks_written := chunks_written + 1;
                }
            }

            state verifying {
                doc /*"Verifying firmware integrity"*/
            }

            state committing {
                doc /*"Copying staging to APP and resetting"*/
            }

            state error {
                doc /*"Error state, awaiting recovery"*/
            }

            // idle -> connected on PING handshake
            transition first idle if active then connected;

            // connected: command dispatch (priority order)
            transition first connected if erase_requested then erasing;
            transition first connected if chunk_ready then receiving;
            transition first connected if commit_requested then verifying;

            // erasing: error takes priority
            transition first erasing if error_flag then error;
            transition first erasing if erase_complete then connected;

            // receiving: error takes priority
            transition first receiving if error_flag then error;
            transition first receiving if chunk_written then connected;

            // verifying: error takes priority
            transition first verifying if error_flag then error;
            transition first verifying if verify_ok then committing;

            // error -> idle (recovery)
            transition first error then idle;
        }

        requirement {
            subject;
            doc /* Bootloader must handle firmware chunks up to 4KB */
        }
    }
}
