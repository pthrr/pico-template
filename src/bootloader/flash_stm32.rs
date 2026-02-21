//! STM32U585 flash storage implementation using `embassy_stm32::flash::Flash`.

use crate::bootloader::{FlashError, FlashStorage};
use crate::config::{BOOTLOADER_CHUNK_SIZE, BOOTLOADER_STAGING_OFFSET, BOOTLOADER_STAGING_SIZE};
use embassy_stm32::flash::{self, Flash};
use vstd::prelude::*;

verus! {

/// STM32U585 erase page size (8 KB).
pub const ERASE_SIZE: u32 = 8192;

/// STM32 platform flash storage backed by `embassy_stm32::flash::Flash`.
pub struct Stm32Flash {
    flash: Flash<'static, flash::Blocking>,
}

impl Stm32Flash {
    pub fn new(flash: Flash<'static, flash::Blocking>) -> Self {
        Self { flash }
    }
}

impl FlashStorage for Stm32Flash {
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
        commit_from_ram()
    }
}

/// RAM-resident commit routine for STM32.
///
/// In production, this would erase APP page-by-page, copy STAGING → APP,
/// then reset.  For this template, we perform a system reset.
#[unsafe(link_section = ".data")]
fn commit_from_ram() -> ! {
    cortex_m::interrupt::disable();

    defmt::info!("Bootloader: commit — resetting into staged firmware");

    #[allow(clippy::cast_possible_truncation)]
    let chunk = BOOTLOADER_CHUNK_SIZE;
    let _ = chunk;

    cortex_m::peripheral::SCB::sys_reset()
}

} // verus!
