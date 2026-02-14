//! Global heap allocator for display support
//!
//! Only compiled when the `display` feature is enabled.
//! Uses embedded-alloc's LlffHeap (Linked List First Fit).

extern crate alloc;

use embedded_alloc::LlffHeap;

#[global_allocator]
static HEAP: LlffHeap = LlffHeap::empty();

/// Heap size per platform:
/// - pico1 (RP2040, 264KB SRAM): 16KB
/// - pico2 (RP2350, 520KB SRAM): 32KB
/// - unoq (STM32U585, 768KB SRAM): 32KB
#[cfg(feature = "pico1")]
const HEAP_SIZE: usize = 16 * 1024;

#[cfg(feature = "pico2")]
const HEAP_SIZE: usize = 32 * 1024;

#[cfg(feature = "unoq")]
const HEAP_SIZE: usize = 32 * 1024;

/// Initialize the heap allocator. Must be called before any heap allocation.
///
/// # Safety
///
/// Must be called exactly once, before any use of `alloc` types.
pub fn init_heap() {
    static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    // SAFETY: Called once at startup before any allocations.
    #[allow(static_mut_refs)]
    unsafe {
        let start = (&raw mut HEAP_MEM) as usize;
        HEAP.init(start, HEAP_SIZE);
    }
    defmt::info!("Heap initialized: {} bytes", HEAP_SIZE);
}
