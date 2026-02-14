# 3. Context and Scope

## C4 Level 1 - System Context

The firmware runs bare-metal on a microcontroller. It interacts with physical hardware (button, LED) and a developer's host machine (debug probe).

```
                    ┌─────────────┐
                    │   Developer │
                    │   (Host PC) │
                    └──────┬──────┘
                           │ SWD / RTT
                           │ (probe-rs)
                           v
┌──────────┐      ┌────────────────┐      ┌──────────┐
│  Button  │─────>│  pico-template │─────>│   LED    │
│  (GPIO)  │ GPIO │   Firmware     │ GPIO │  (GPIO)  │
└──────────┘      └────────────────┘      └──────────┘
                     MCU target
```

### External Actors

| Actor | Description | Interface |
|-------|-------------|-----------|
| User | Presses physical button | GPIO input (active-low with pull-up) |
| Developer | Flashes firmware, reads logs | SWD debug probe (probe-rs) / UF2 drag-and-drop / remoteocd (UNO Q) |

### External Systems

| System | Description | Interface |
|--------|-------------|-----------|
| LED | Status indicator (heartbeat) | GPIO output |
| Button | User input | GPIO input with software debounce |

## Business Context

| Communication Partner | Input | Output |
|----------------------|-------|--------|
| Button hardware | GPIO level (high/low) | `ButtonMessage::Pressed` / `Released` to control actor |
| LED hardware | - | GPIO set high/low from maintenance actor |
| Host PC | - | defmt log frames over RTT |

## Technical Context

| Channel | Protocol | Description |
|---------|----------|-------------|
| SWD | ARM Serial Wire Debug | Flash, debug, RTT log transport |
| GPIO | Digital I/O | Button input (pull-up, active-low), LED output |
| USB (optional) | CDC ACM | USB device support via `embassy-usb` (feature-gated) |
| remoteocd + ADB | OpenOCD over ADB | Flash transport for Arduino UNO Q |
