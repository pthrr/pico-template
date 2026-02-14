# 4. Solution Strategy

## Technology Decisions

| Decision | Details |
|----------|---------|
| Language | Rust `no_std` - memory safety without OS, zero-cost abstractions |
| Async runtime | Embassy - cooperative async/await on bare-metal, multi-core support |
| Actor model | State-machine actors communicating via typed `embassy_sync::Channel` |
| Model-driven | SysML 2 → Rust codegen for actor state machines |
| Formal verification | TLA+ specs for each actor, checked with TLC |
| Configuration | CUE schema → JSON → Rust constants (build-time codegen in `build.rs`) |
| Multi-target | Cargo feature flags (`pico1`, `pico2`, `unoq`) select HAL + linker script |

## Top-Level Decomposition

The system is split into three layers:

1. **Platform layer** (`main_rp2040.rs`, `main_unoq.rs`) - Hardware init, GPIO pin assignment, Embassy executor setup, core affinity. Platform-specific Embassy task wrappers (Embassy tasks cannot be generic).

2. **Actor layer** (`tasks.rs`, `*_hw.rs`) - Platform-agnostic async task functions that drive actor state machines. Each `*_hw.rs` struct composes a generated state machine with hardware resources and channel references.

3. **Generated layer** (`src/generated/`) - Pure state machine logic generated from SysML models. No hardware dependencies, no Embassy dependencies. Stepped synchronously via `.step()`.

## Key Quality Approaches

| Quality Goal | Approach |
|-------------|----------|
| Deterministic timing | Dedicated core for control task (RP2040 Core 0), deadline monitoring with defmt logging on overrun |
| Portability | Custom `hal::OutputPin` / `hal::InputPin` traits with per-target `cfg`-gated impls; generic task functions parameterized over these traits |
| Correctness | SysML as single source of truth → codegen ensures model-code consistency; TLA+ model checking proves safety invariants |
| Maintainability | `define_channels!` macro eliminates channel boilerplate; CUE config schema validates pin assignments and timing parameters |
