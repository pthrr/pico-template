# 1. Introduction and Goals

## Requirements Overview

pico-template is a multi-target embedded firmware template implementing an actor-based real-time control system. The system manages a control loop, button input, and LED maintenance across three hardware platforms using a shared, platform-agnostic codebase.

| ID | Requirement | Priority |
|----|-------------|----------|
| R1 | Real-time control loop at 1 kHz (1 ms period, max 50 us jitter) | High |
| R2 | Button input with debounce (5-cycle threshold, 10 ms debounce period) | Medium |
| R3 | LED heartbeat toggle at 1 Hz via maintenance actor (10 Hz tick rate) | Medium |
| R4 | Multi-target support: Pico 1 (RP2040), Pico 2 (RP2350), Arduino UNO Q (STM32U585) | High |
| R5 | Actor state machines generated from SysML 2 models | High |
| R6 | Formal verification of actor state machines via TLA+ | Medium |
| R7 | Multi-core task distribution on RP2040 (Core 0: control, Core 1: peripherals) | Medium |

## Quality Goals

| Priority | Quality Goal | Scenario |
|----------|-------------|----------|
| 1 | Deterministic timing | Control loop executes within 800 us, with jitter below 50 us |
| 2 | Portability | Same actor logic runs on RP2040, RP2350, and STM32U585 without changes |
| 3 | Correctness | State machine behavior is formally specified (SysML) and verified (TLA+) |
| 4 | Maintainability | Actor state machines are generated from models, not hand-written |

## Stakeholders

| Role | Expectations |
|------|-------------|
| Firmware developer | Clear platform abstraction, easy to add new targets |
| System engineer | SysML models as single source of truth for actor behavior |
| QA / Verification | TLA+ specs prove safety properties of state machines |
