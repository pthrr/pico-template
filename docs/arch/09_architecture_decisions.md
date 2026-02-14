# 9. Architecture Decisions

## ADR-001: Actor Model with Channel-Based Communication

**Status:** accepted

**Context:** The system has multiple concurrent concerns (real-time control, button input, LED maintenance) with different timing requirements. These need to interact without tight coupling or shared mutable state.

**Decision:** Use an actor model where each concern is an independent async task communicating via typed `embassy_sync::Channel`. Messages are defined as enums/structs in `messages.rs`. A `define_channels!` macro creates static channel instances.

**Consequences:** Actors are decoupled and testable in isolation. Channel capacity is fixed at compile time (4 for button, 2 for maintenance). Non-blocking `try_send`/`try_receive` means messages can be dropped under overload rather than blocking a time-critical task.

---

## ADR-002: SysML-to-Rust Code Generation

**Status:** accepted

**Context:** Actor state machines need to be correct, formally verifiable, and consistent between the model and the implementation.

**Decision:** Define actors in SysML 2 (`model/*.sysml`), generate Rust state machine code via `tools/sysml_codegen.py` into `src/generated/`. Generated code is pure (no hardware deps) and exposes a `step()` method. Hardware integration is done in separate `*_hw.rs` wrappers.

**Consequences:** Models are the single source of truth. Generated code should not be manually edited. The codegen tool must be maintained alongside SysML schema changes. Clean separation between generated logic and hardware integration.

---

## ADR-003: Multi-Core Task Distribution on RP2040

**Status:** accepted

**Context:** The RP2040 has two Cortex-M0+ cores. The control task has a hard 1 kHz timing requirement; maintenance and button tasks are less timing-sensitive.

**Decision:** Run the control task on Core 0 (main executor). Spawn a secondary Embassy `Executor` on Core 1 for maintenance and button tasks. Use `CriticalSectionRawMutex` channels for cross-core communication.

**Consequences:** Control task gets deterministic scheduling without interference from lower-priority tasks. Core 1 stack is statically allocated (4 KB). The secondary executor requires a `StaticCell`. Single-core targets (STM32U585) run all tasks on one core with the same channel primitives.

---

## ADR-004: CUE for Build-Time Configuration

**Status:** accepted

**Context:** Pin assignments and timing parameters differ per target. These should be validated at build time and available as Rust constants.

**Decision:** Define a CUE schema (`data/config/config.cue`) with typed, constrained fields per platform. `build.rs` exports the selected platform config to JSON, then generates `config.rs` with Rust constants.

**Consequences:** Type-safe configuration with schema validation (e.g., `led_pin: uint8 & >=0 & <=28`). Adding a new platform requires adding a CUE stanza. The `cue` CLI must be available at build time.

---

## ADR-005: Custom HAL Traits Instead of embedded-hal

**Status:** accepted

**Context:** The project uses `embedded-hal` 1.0 as a dependency but the Embassy GPIO types don't always align perfectly with the needed interface. A thin abstraction is needed for the actor layer.

**Decision:** Define minimal `OutputPin` and `InputPin` traits in `hal.rs` with `cfg`-gated implementations for each target's Embassy GPIO types.

**Consequences:** Actor hardware structs are generic over these traits, enabling platform-agnostic task logic. The traits are intentionally minimal (set_high/set_low/is_low) to reduce abstraction overhead. New targets require adding a new `cfg`-gated impl block.

---

## ADR-006: TLA+ for Formal Verification

**Status:** accepted

**Context:** State machine correctness is critical for the real-time control system. Unit tests cover individual transitions but cannot exhaustively prove safety properties.

**Decision:** Write TLA+ specifications (`model/tla/*.tla`) mirroring the SysML actor state machines. Use TLC model checker to verify type invariants and safety properties. State constraints bound the model checking space.

**Consequences:** Every actor has a corresponding `.tla` and `.cfg` file. Safety properties like `state \in States` are mechanically verified. TLA+ specs must be kept in sync with SysML models (currently manual).
