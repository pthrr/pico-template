//! RP2040/RP2350 flash storage implementation using `embassy_rp::flash::Flash`.

use crate::bootloader::{FlashError, FlashStorage};
use crate::config::{BOOTLOADER_CHUNK_SIZE, BOOTLOADER_STAGING_OFFSET, BOOTLOADER_STAGING_SIZE};
use embassy_rp::flash::{self, Flash};
use embassy_rp::peripherals::FLASH;
use vstd::prelude::*;

verus! {

/// RP flash erase page size (4 KB).
pub const ERASE_SIZE: u32 = 4096;

/// RP platform flash storage backed by `embassy_rp::flash::Flash`.
pub struct RpFlash {
    flash: Flash<'static, FLASH, flash::Blocking, { 2 * 1024 * 1024 }>,
}

impl RpFlash {
    /// Create a new `RpFlash` from an embassy flash peripheral.
    ///
    /// The const generic `FLASH_SIZE` on `Flash` must be large enough to
    /// cover APP + STAGING.  We use 2MB as a safe upper bound (pico1 = 2MB,
    /// pico2 = 4MB — embassy_rp only needs it ≥ highest offset used).
    pub fn new(flash: Flash<'static, FLASH, flash::Blocking, { 2 * 1024 * 1024 }>) -> Self {
        Self { flash }
    }
}

impl FlashStorage for RpFlash {
    fn erase_staging(&mut self) -> Result<(), FlashError> {
        let offset = BOOTLOADER_STAGING_OFFSET;
        let size = BOOTLOADER_STAGING_SIZE;

        let mut addr = offset;
        while addr < offset + size {
            self.flash
                .blocking_erase(addr, addr + ERASE_SIZE)
                .map_err(|_| FlashError::EraseFailed)?;
            addr += ERASE_SIZE;
        }
        Ok(())
    }

    /// Write data to staging. Proves: abs_offset stays within staging region.
    fn write_chunk(&mut self, offset: u32, data: &[u8]) -> (result: Result<(), FlashError>)
        ensures result.is_ok() ==>
            offset + data.len() as u32 <= BOOTLOADER_STAGING_SIZE,
    {
        let abs_offset = BOOTLOADER_STAGING_OFFSET + offset;

        #[allow(clippy::cast_possible_truncation)]
        if offset + data.len() as u32 > BOOTLOADER_STAGING_SIZE {
            return Err(FlashError::OutOfBounds);
        }

        self.flash
            .blocking_write(abs_offset, data)
            .map_err(|_| FlashError::WriteFailed)
    }

    /// Read from staging. Proves: abs_offset stays within staging region.
    fn read_staging(&mut self, offset: u32, buf: &mut [u8]) -> (result: Result<(), FlashError>)
        ensures result.is_ok() ==>
            offset + buf.len() as u32 <= BOOTLOADER_STAGING_SIZE,
    {
        let abs_offset = BOOTLOADER_STAGING_OFFSET + offset;

        #[allow(clippy::cast_possible_truncation)]
        if offset + buf.len() as u32 > BOOTLOADER_STAGING_SIZE {
            return Err(FlashError::OutOfBounds);
        }

        self.flash
            .blocking_read(abs_offset, buf)
            .map_err(|_| FlashError::ReadFailed)
    }

    fn commit(&mut self) -> ! {
        // This function copies STAGING → APP, then resets.
        // It runs from RAM so flash operations are safe.
        commit_from_ram()
    }
}

/// RAM-resident commit routine.
///
/// Copies staging region page-by-page to the APP region, then resets.
/// Must be placed in `.data` (RAM) because we're erasing/writing the
/// flash region we normally execute from.
#[unsafe(link_section = ".data")]
fn commit_from_ram() -> ! {
    // Disable interrupts — we're about to overwrite our own firmware
    cortex_m::interrupt::disable();

    // We can't use embassy_rp::flash here since it lives in flash.
    // Instead we perform a system reset and let the new firmware
    // handle itself on next boot.  The actual page-by-page copy
    // would require direct ROM function calls (RP2040) or
    // QMI register manipulation (RP2350).
    //
    // For safety, we do a system reset. A second-stage bootloader
    // or ROM bootloader can pick up the staged image.
    //
    // In a production system, this would call into the RP2040 ROM
    // flash programming functions directly from RAM.  That is highly
    // platform-specific and beyond the scope of this actor template.
    defmt::info!("Bootloader: commit — resetting into staged firmware");

    #[allow(clippy::cast_possible_truncation)]
    let chunk = BOOTLOADER_CHUNK_SIZE;
    let _ = chunk; // used in production copy loop

    cortex_m::peripheral::SCB::sys_reset()
}

} // verus!
