// Bootloader actor - firmware update via USB CDC (RP) or UART (STM32)
// Runs on Core 1 (RP) or main executor (STM32)
package Bootloader {
    private import ScalarValues::*;

    action def reset_state {
        bytes_received := 0;
        chunks_written := 0;
        total_size := 0;
        error_code := 0;
        error_flag := false;
    }

    action def clear_erase {
        erase_requested := false;
        erase_complete := false;
    }

    action def clear_receive {
        chunk_ready := false;
        chunk_written := false;
    }

    action def clear_verify {
        commit_requested := false;
        verify_ok := false;
    }

    action def count_chunk {
        chunks_written := chunks_written + 1;
    }

    state def Idle {
        doc /*"Waiting for bootloader activation"*/
        entry action : reset_state;
    }

    state def Connected {
        doc /*"Connection established, awaiting erase command"*/
    }

    state def Erasing {
        doc /*"Erasing staging flash area"*/
        entry action : clear_erase;
    }

    state def Erased {
        doc /*"Erase complete, awaiting data transfer or commit"*/
        entry action : clear_receive;
    }

    state def Receiving {
        doc /*"Writing firmware chunk to staging"*/
        do action : count_chunk;
    }

    state def Verifying {
        doc /*"Verifying firmware integrity"*/
        entry action : clear_verify;
    }

    state def Committing {
        doc /*"Copying staging to APP and resetting"*/
    }

    state def Error {
        doc /*"Error state, awaiting recovery"*/
    }

    part def BootloaderActor {
        attribute active : Boolean = false;
        attribute erase_requested : Boolean = false;
        attribute erase_complete : Boolean = false;
        attribute chunk_ready : Boolean = false;
        attribute chunk_written : Boolean = false;
        attribute commit_requested : Boolean = false;
        attribute verify_ok : Boolean = false;
        attribute error_flag : Boolean = false;

        attribute bytes_received : Integer = 0;
        attribute chunks_written : Integer = 0;
        attribute total_size : Integer = 0;
        attribute error_code : Integer = 0;

        attribute core : Integer = 1;
        attribute priority : Integer = 3;

        exhibit state machine {
            state : Idle;
            state : Connected;
            state : Erasing;
            state : Erased;
            state : Receiving;
            state : Verifying;
            state : Committing;
            state : Error;

            // idle -> connected on PING handshake
            transition first Idle accept active then Connected;

            // connected: only accepts erase command
            transition first Connected accept erase_requested then Erasing;

            // erasing: error takes priority
            transition first Erasing accept error_flag then Error;
            transition first Erasing accept erase_complete then Erased;

            // erased: commit takes priority (host decides when data transfer is done)
            transition first Erased accept error_flag then Error;
            transition first Erased accept commit_requested then Verifying;
            transition first Erased accept chunk_ready then Receiving;

            // receiving: error takes priority
            transition first Receiving accept error_flag then Error;
            transition first Receiving accept chunk_written then Erased;

            // verifying: error takes priority
            transition first Verifying accept error_flag then Error;
            transition first Verifying accept verify_ok then Committing;

            // committing -> idle (reset after successful commit)
            transition first Committing then Idle;

            // error -> idle (recovery)
            transition first Error then Idle;
        }

        requirement {
            subject;
            doc /* Bootloader must handle firmware chunks up to 4KB */
        }
    }
}
