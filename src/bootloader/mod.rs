//! Bootloader module — firmware update via USB CDC (RP) or UART (STM32)
//!
//! Provides a `FlashStorage` trait abstraction and a custom serial protocol
//! for receiving and committing firmware images.

pub mod protocol;

#[cfg(any(feature = "pico1", feature = "pico2"))]
pub mod flash_rp;

#[cfg(feature = "unoq")]
pub mod flash_stm32;

/// Errors from flash operations.
#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum FlashError {
    /// Erase operation failed
    EraseFailed,
    /// Write operation failed
    WriteFailed,
    /// Read operation failed
    ReadFailed,
    /// CRC mismatch on chunk
    CrcMismatch,
    /// Offset out of bounds for staging region
    OutOfBounds,
}

/// Abstraction over platform-specific flash operations.
///
/// All offsets are relative to the start of the staging region.
pub trait FlashStorage {
    /// Erase the entire staging region.
    fn erase_staging(&mut self) -> Result<(), FlashError>;

    /// Write a chunk of data at the given offset within staging.
    fn write_chunk(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError>;

    /// Read data from the staging region at the given offset.
    fn read_staging(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), FlashError>;

    /// Copy staging → APP and reset. This function never returns.
    fn commit(&mut self) -> !;
}
