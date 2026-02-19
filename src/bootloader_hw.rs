//! Bootloader actor implementation with hardware integration
//!
//! Follows the same pattern as `maintenance_hw.rs`: wraps the generated
//! `BootloaderActor` state machine and drives flash operations based on
//! parsed protocol commands.

use crate::bootloader::protocol::{Command, Response, ResponseError};
use crate::bootloader::{FlashError, FlashStorage};
use crate::config::{BOOTLOADER_CHUNK_SIZE, BOOTLOADER_STAGING_SIZE};
use crate::generated::bootloader::BootloaderActor;
use crc::{CRC_16_IBM_3740, CRC_32_ISO_HDLC, Crc};

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_3740);
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

/// Hardware-integrated bootloader actor.
pub struct BootloaderActorHw<F: FlashStorage> {
    pub actor: BootloaderActor,
    pub flash: F,
}

impl<F: FlashStorage> BootloaderActorHw<F> {
    pub fn new(flash: F) -> Self {
        Self {
            actor: BootloaderActor::new(),
            flash,
        }
    }

    /// Get the platform name for INFO responses.
    fn platform_name() -> &'static str {
        #[cfg(feature = "pico1")]
        {
            "pico1"
        }
        #[cfg(feature = "pico2")]
        {
            "pico2"
        }
        #[cfg(feature = "unoq")]
        {
            "unoq"
        }
    }

    /// Process a parsed protocol command.
    ///
    /// Sets actor flags, calls `step()`, performs flash I/O, returns response.
    pub fn process_command(&mut self, cmd: Command, payload: &[u8]) -> Response {
        match cmd {
            Command::Ping => {
                self.actor.active = true;
                self.actor.step();
                defmt::info!("Bootloader: PING → connected");
                Response::Pong
            }
            Command::Info => Response::Info {
                platform: Self::platform_name(),
                staging_size: BOOTLOADER_STAGING_SIZE,
                chunk_size: u32::from(BOOTLOADER_CHUNK_SIZE),
            },
            Command::Erase => {
                self.actor.erase_requested = true;
                self.actor.step(); // → Erasing

                match self.flash.erase_staging() {
                    Ok(()) => {
                        self.actor.erase_complete = true;
                        self.actor.step(); // → Connected
                        defmt::info!("Bootloader: staging erased");
                        Response::Ok
                    }
                    Err(e) => {
                        self.actor.error_flag = true;
                        self.actor.step(); // → Error
                        defmt::error!("Bootloader: erase failed");
                        Response::Err(ResponseError::Flash(e))
                    }
                }
            }
            Command::Flash { offset, len, crc16 } => {
                // Verify CRC16 of the received chunk
                let data = &payload[..usize::from(len)];
                let computed = CRC16.checksum(data);
                if computed != crc16 {
                    defmt::error!(
                        "Bootloader: CRC16 mismatch (expected={}, got={})",
                        crc16,
                        computed
                    );
                    self.actor.error_flag = true;
                    self.actor.step();
                    return Response::Err(ResponseError::BadCrc);
                }

                self.actor.chunk_ready = true;
                self.actor.step(); // → Receiving

                match self.flash.write_chunk(offset, data) {
                    Ok(()) => {
                        self.actor.chunk_written = true;
                        self.actor.bytes_received += i32::from(len);
                        self.actor.step(); // → Connected
                        defmt::debug!("Bootloader: wrote {} bytes at offset {}", len, offset);
                        Response::Ok
                    }
                    Err(e) => {
                        self.actor.error_flag = true;
                        self.actor.step(); // → Error
                        defmt::error!("Bootloader: write failed at offset {}", offset);
                        Response::Err(ResponseError::Flash(e))
                    }
                }
            }
            Command::Commit { total_size, crc32 } => {
                self.actor.commit_requested = true;
                self.actor.total_size = total_size as i32;
                self.actor.step(); // → Verifying

                // Verify CRC32 over the entire staged image
                match self.verify_staged_crc32(total_size, crc32) {
                    Ok(()) => {
                        self.actor.verify_ok = true;
                        self.actor.step(); // → Committing
                        defmt::info!("Bootloader: verified {} bytes, committing...", total_size);
                        // This never returns
                        self.flash.commit();
                    }
                    Err(e) => {
                        self.actor.error_flag = true;
                        self.actor.step(); // → Error
                        Response::Err(ResponseError::Flash(e))
                    }
                }
            }
        }
    }

    /// Read back staged firmware and verify CRC32.
    fn verify_staged_crc32(&mut self, total_size: u32, expected: u32) -> Result<(), FlashError> {
        let mut digest = CRC32.digest();
        let mut read_buf = [0u8; 256];
        let mut offset: u32 = 0;

        while offset < total_size {
            let remaining = total_size - offset;
            let chunk_len = if remaining < 256 {
                remaining as usize
            } else {
                256
            };
            self.flash
                .read_staging(offset, &mut read_buf[..chunk_len])?;
            digest.update(&read_buf[..chunk_len]);
            offset += chunk_len as u32;
        }

        let computed = digest.finalize();
        if computed == expected {
            Ok(())
        } else {
            defmt::error!(
                "Bootloader: CRC32 mismatch (expected={}, got={})",
                expected,
                computed
            );
            Err(FlashError::CrcMismatch)
        }
    }
}
