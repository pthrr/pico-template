# 7. Deployment View

## Infrastructure

```
┌─────────────────────────────────────────────────────────────────┐
│                        Host PC                                   │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │ Cargo/Rustc  │  │  probe-rs    │  │ remoteocd + ADB       │  │
│  │ (build)      │  │ (flash/debug)│  │ (UNO Q flash)         │  │
│  └──────┬───────┘  └──────┬───────┘  └───────────┬───────────┘  │
│         │                 │ SWD                   │ USB/ADB      │
└─────────┼─────────────────┼───────────────────────┼──────────────┘
          │                 │                       │
          v                 v                       v
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────┐
│ Pico 1 (RP2040) │  │ Pico 2 (RP2350) │  │ Arduino UNO Q       │
│                 │  │                 │  │ (STM32U585)         │
│ BOOT2: 256 B    │  │ BOOT2: 256 B    │  │                     │
│ FLASH: 2 MB     │  │ FLASH: 4 MB     │  │ FLASH: 2 MB         │
│ RAM:   264 KB   │  │ RAM:   520 KB   │  │ RAM:   768 KB        │
│                 │  │                 │  │                     │
│ Cortex-M0+      │  │ Cortex-M33      │  │ Cortex-M33           │
│ Dual-core       │  │ Dual-core       │  │ Single-core          │
│ thumbv6m        │  │ thumbv8m        │  │ thumbv8m             │
└─────────────────┘  └─────────────────┘  └─────────────────────┘
```

## Memory Maps

### Pico 1 (RP2040)

| Region | Origin | Size |
|--------|--------|------|
| BOOT2 | `0x1000_0000` | 256 B |
| FLASH | `0x1000_0100` | 2 MB - 256 B |
| RAM | `0x2000_0000` | 264 KB |

### Pico 2 (RP2350)

| Region | Origin | Size |
|--------|--------|------|
| BOOT2 | `0x1000_0000` | 256 B |
| FLASH | `0x1000_0100` | 4 MB - 256 B |
| RAM | `0x2000_0000` | 520 KB |

### Arduino UNO Q (STM32U585)

| Region | Origin | Size |
|--------|--------|------|
| FLASH | `0x0800_0000` | 2 MB |
| RAM | `0x2000_0000` | 768 KB (SRAM1 192 KB + SRAM2 64 KB + SRAM3 512 KB) |

## Build & Flash Process

| Target | Build Command | Flash Method |
|--------|--------------|-------------|
| Pico 1 | `task build_pico1` | `probe-rs run --chip RP2040` via SWD |
| Pico 2 | `task build_pico2` | `probe-rs run --chip RP2350` via SWD |
| UNO Q | `task build_unoq` | `task flash_unoq` (remoteocd over ADB, auto-detects serial) |

### Build Details

| Target | Cargo Feature | Rust Target Triple | Linker Script |
|--------|--------------|-------------------|---------------|
| Pico 1 | `pico1` (default) | `thumbv6m-none-eabi` | `memory-pico1.x` |
| Pico 2 | `pico2` | `thumbv8m.main-none-eabihf` | `memory-pico2.x` |
| UNO Q | `unoq` | `thumbv8m.main-none-eabihf` | `memory-stm32u585.x` |

### Release Profile

| Setting | Value | Rationale |
|---------|-------|-----------|
| LTO | enabled | Whole-program optimization, smaller binary |
| opt-level | `s` | Optimize for size |
| codegen-units | 1 | Better optimization at cost of compile time |
| debug | true | Debug symbols available in release builds |
