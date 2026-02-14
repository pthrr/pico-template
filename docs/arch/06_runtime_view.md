# 6. Runtime View

## Scenario: RP2040 Boot and Task Distribution

On RP2040 (pico1/pico2), tasks are distributed across two cores:

```
Core 0                          Core 1
  │                               │
  │ embassy_rp::init()            │
  │ GPIO setup (LED, button)      │
  │                               │
  │ spawn control_task ──┐        │
  │                      │        │
  │ spawn_core1() ───────────────>│ Executor::new()
  │                      │        │
  │                      │        │ spawn maintenance_task
  │                      │        │ spawn button_task
  │                      │        │
  │  ┌───────────────┐   │   ┌────┴──────────────┐
  │  │ control_task  │   │   │ maintenance_task   │
  │  │ 1 kHz loop    │   │   │ 10 Hz loop         │
  │  │               │   │   │                    │
  │  │ receive from  │<──────│ send Maintenance   │
  │  │  maintenance  │   │   │  Message           │
  │  │               │   │   └────────────────────┘
  │  │ receive from  │   │
  │  │  button       │<──────┐
  │  │               │   │   │
  │  └───────────────┘   │   │ ┌────────────────┐
  │                      │   │ │ button_task    │
  │                      │   │ │ 10 ms poll     │
  │                      │   └─│ send Button    │
  │                      │     │  Message       │
  │                      │     └────────────────┘
  v                      v
```

## Scenario: UNO Q Boot (Single-Core)

On STM32U585 (Arduino UNO Q), all tasks run on one core. Currently a simple blink demo:

```
Core 0
  │
  │ embassy_stm32::init()
  │ Output::new(PH11, High)  // LED off (active-low)
  │
  │ spawn blink_task
  │
  │  ┌───────────────────┐
  │  │ blink_task         │
  │  │ loop:              │
  │  │   LED LOW  (on)    │
  │  │   wait 500 ms      │
  │  │   LED HIGH (off)   │
  │  │   wait 500 ms      │
  │  └───────────────────┘
  v
```

## Scenario: Button Press → Control Response

```
ButtonActorHw                          ControlActorHw
     │                                      │
     │ poll: button_pin.is_low()            │
     │ actor.pressed = true                 │
     │                                      │
     │ step(): Idle → Debouncing            │
     │ step(): debounce_counter++           │
     │ ... (5 cycles @ 10 ms = 50 ms)      │
     │ step(): Debouncing → PressedState    │
     │ step(): PressedState → Notifying     │
     │                                      │
     │──── ButtonMessage::Pressed ─────────>│
     │                                      │ step(): process message
     │ step(): Notifying → Released         │
     │                                      │
     │──── ButtonMessage::Released ────────>│
     │                                      │ step(): process message
     │ step(): Released → Idle              │
     │                                      │
```

## Scenario: Maintenance LED Toggle

```
MaintenanceActorHw                     ControlActorHw
     │                                      │
     │ step(): Idle → Checking              │
     │   tick_count++                       │
     │                                      │
     │ (tick_count < 10)                    │
     │ step(): Checking → Reporting         │
     │──── MaintenanceMessage ─────────────>│
     │ step(): Reporting → Idle             │
     │                                      │
     │ ... (repeat 10x = 1 second)          │
     │                                      │
     │ (tick_count >= 10)                   │
     │ step(): Checking → Toggling          │
     │   led_state = !led_state             │
     │   GPIO: set_high() or set_low()     │
     │   tick_count = 0                     │
     │ step(): Toggling → Reporting         │
     │──── MaintenanceMessage ─────────────>│
     │                                      │
```
