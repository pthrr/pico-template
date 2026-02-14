# 12. Glossary

| Term | Definition |
|------|-----------|
| Actor | An independent concurrent unit with its own state machine, communicating via message channels |
| BSP | Board Support Package - board-level abstractions on top of the HAL |
| CUE | Configuration language with built-in validation; used here for typed build-time config |
| defmt | Deferred formatting - embedded logging framework that keeps format strings on the host |
| Embassy | Async runtime for embedded Rust; provides executor, timers, sync primitives, and HAL drivers |
| flip-link | Linker wrapper that flips the memory layout so stack overflows cause HardFault instead of silent corruption |
| HAL | Hardware Abstraction Layer - trait-based interface decoupling logic from hardware registers |
| no_std | Rust without the standard library; required for bare-metal targets without an OS |
| probe-rs | Debug probe toolkit for ARM targets; handles flashing, debugging, and RTT log reading |
| remoteocd | Arduino's remote OpenOCD tool for flashing over ADB (used for UNO Q) |
| RP2040 | Raspberry Pi dual-core Arm Cortex-M0+ microcontroller (Pico 1) |
| RP2350 | Raspberry Pi dual-core Arm Cortex-M33 / RISC-V microcontroller (Pico 2) |
| RTT | Real-Time Transfer - in-memory debug channel between MCU and host via SWD |
| STM32U585 | ST Microelectronics Arm Cortex-M33 ultra-low-power microcontroller (Arduino UNO Q) |
| SWD | Serial Wire Debug - two-wire debug interface for ARM Cortex processors |
| SysML 2 | Systems Modeling Language v2 - used here to define actor state machines as models |
| TLA+ | Temporal Logic of Actions - formal specification language for verifying concurrent systems |
| TLC | TLA+ model checker - exhaustively checks TLA+ specifications against invariants |
| UF2 | USB Flashing Format - drag-and-drop firmware update protocol (RP2040/RP2350) |
