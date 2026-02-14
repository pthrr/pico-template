# 8. Crosscutting Concepts

## Model-Driven Development

Actor state machines follow a three-stage pipeline:

1. **SysML 2 models** (`model/*.sysml`) - Define actor states, transitions, guards, actions, and timing constraints as the single source of truth.
2. **Rust codegen** (`tools/sysml_codegen.py`) - Generates `src/generated/*.rs` with pure state machine structs and `step()` methods. No hardware or Embassy dependencies in generated code.
3. **TLA+ verification** (`model/tla/*.tla`) - Formal specifications of the same state machines, verified with TLC model checker to prove safety invariants (e.g., states remain within defined set).

## Configuration

Build-time configuration via CUE:

1. `data/config/config.cue` defines a typed schema with per-platform pin assignments and timing parameters (LED pin, button pin, control period, debounce timing).
2. `build.rs` runs `cue export` → JSON → generates `config.rs` with Rust constants.
3. Platform code references constants like `LED_PIN`, `CONTROL_PERIOD_MS`, `BUTTON_DEBOUNCE_MS` directly.

## Error Handling

- **Control actor**: Monitors execution deadline (1 ms). On overrun, logs via `defmt::info!` but continues (soft deadline). The generated state machine has an explicit `Error` state that disables control output and resets to `Initializing`.
- **Channel communication**: Uses `try_send()` / `try_receive()` (non-blocking). Messages are silently dropped if the channel is full, preventing task blocking.
- **Panic handling**: `panic-probe` halts the MCU and outputs the panic message via defmt/RTT.

## Logging / Diagnostics

- **defmt** - Deferred formatting: format strings stay on the host, only indices + arguments are sent over RTT. Near-zero runtime overhead.
- **defmt-rtt** - Transport layer over SWD debug probe.
- **Log levels**: `DEFMT_LOG=trace` (set in `.cargo/config.toml`).
- All message types derive `defmt::Format` for structured logging.

## Memory Management

- `#![no_std]` - No standard library, no OS, no dynamic allocator.
- All state is statically allocated: static channels (`define_channels!` macro), `StaticCell` for the secondary executor, fixed-size `Stack<4096>` for Core 1.
- `heapless` collections available for bounded data structures.
- `flip-link` linker wrapper flips the memory layout so stack overflows trigger a HardFault instead of silently corrupting data.

## Concurrency Model

- **Embassy async executor** - Cooperative multitasking via `async`/`await`. Tasks yield at `.await` points (timers, channel operations).
- **Multi-core (RP2040)**: Core 0 runs the main Embassy executor with the control task. Core 1 runs a secondary `Executor` (via `StaticCell`) with maintenance and button tasks. Channels use `CriticalSectionRawMutex` for cross-core safety.
- **Single-core (STM32U585)**: All tasks on one executor. Same channel mutex works for single-core.
- **No preemption**: Tasks are cooperative. The control task monitors its own deadline compliance.

## Platform Abstraction

Two custom traits in `hal.rs` abstract GPIO access:

- `OutputPin` - `set_high()`, `set_low()`
- `InputPin` - `is_low()`, `is_high()`

Per-target impls are `cfg`-gated:
- `pico1` / `pico2` → `embassy_rp::gpio::{Output, Input}`
- `unoq` → `embassy_stm32::gpio::{Output, Input}`

Actor hardware structs (`ButtonActorHw<I: InputPin>`, `MaintenanceActorHw<O: OutputPin>`) are generic over these traits, keeping task logic platform-independent. Embassy tasks cannot be generic, so each `main_*.rs` defines concrete task wrappers that delegate to the generic functions in `tasks.rs`.

## Testing Strategy

- **On-target integration tests** (`tests/integration.rs`) - Uses `defmt_test` harness. Tests run on real hardware via probe-rs. Cover actor initialization and state machine stepping.
- **Formal verification** - TLA+ specs (`model/tla/`) model-checked with TLC to verify state machine safety properties.
- **Build check** - `task test` runs `cargo check --lib --features pico1` as a fast CI gate.
