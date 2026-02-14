# 5. Building Block View

## C4 Level 2 - Container Diagram

The system is a single firmware binary. "Containers" here are logical layers within the binary.

```
┌─────────────────────────────────────────────────────────────────┐
│                        pico-template                            │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Platform Layer (main_*.rs)                  │   │
│  │   main_rp2040.rs          main_unoq.rs                  │   │
│  │   - embassy_rp GPIO       - embassy_stm32 GPIO          │   │
│  │   - dual-core executor    - single-core executor        │   │
│  │   - concrete task wrappers                              │   │
│  └──────────────────────┬──────────────────────────────────┘   │
│                         │ spawns tasks                          │
│  ┌──────────────────────v──────────────────────────────────┐   │
│  │              Actor Layer (tasks.rs + *_hw.rs)            │   │
│  │                                                          │   │
│  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │   │
│  │   │ ControlActor │  │ ButtonActor  │  │ Maintenance  │ │   │
│  │   │   Hw         │  │   Hw         │  │   ActorHw    │ │   │
│  │   │  (1 kHz)     │  │  (polling)   │  │  (10 Hz)     │ │   │
│  │   └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │   │
│  │          │                 │                  │          │   │
│  │          │  Channel<ButtonMessage, 4>         │          │   │
│  │          │<────────────────┘                  │          │   │
│  │          │  Channel<MaintenanceMessage, 2>    │          │   │
│  │          │<──────────────────────────────────-┘          │   │
│  └──────────┼───────────────────────────────────────────────┘   │
│             │ .step()                                           │
│  ┌──────────v───────────────────────────────────────────────┐   │
│  │          Generated Layer (src/generated/)                │   │
│  │                                                          │   │
│  │   ButtonActor          RealtimeControlActor              │   │
│  │   state machine        state machine                     │   │
│  │                                                          │   │
│  │   MaintenanceActor                                       │   │
│  │   state machine                                          │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Support                                     │   │
│  │   hal.rs        messages.rs      config.rs               │   │
│  │   (GPIO traits) (channel types)  (CUE constants)         │   │
│  │   actor_channels.rs                                      │   │
│  │   (define_channels! macro)                               │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

| Layer | Description | Key Files |
|-------|-------------|-----------|
| Platform | HW init, pin config, executor setup, task spawning | `main_rp2040.rs`, `main_unoq.rs` |
| Actor | Platform-agnostic task loops driving state machines | `tasks.rs`, `button_hw.rs`, `control_hw.rs`, `maintenance_hw.rs` |
| Generated | Pure state machines from SysML codegen | `generated/button.rs`, `generated/control.rs`, `generated/maintenance.rs` |
| Support | Shared traits, types, config, macros | `hal.rs`, `messages.rs`, `config.rs`, `actor_channels.rs` |

## C4 Level 3 - Component Diagram

### Control Actor

```
┌─────────────────────────────────────────────┐
│              ControlActorHw                  │
│                                              │
│  ┌────────────────────────┐                  │
│  │ RealtimeControlActor   │  (generated)     │
│  │ States: Initializing → │                  │
│  │   Running → Processing │                  │
│  │   → Outputting → ...   │                  │
│  │   Error → Initializing │                  │
│  └────────────────────────┘                  │
│                                              │
│  Inputs:                                     │
│    Channel<ButtonMessage, 4>      (receive)  │
│    Channel<MaintenanceMessage, 2> (receive)  │
│                                              │
│  Rate: 1 kHz  |  Core: 0 (RP2040)           │
└──────────────────────────────────────────────┘
```

### Button Actor

```
┌─────────────────────────────────────────────┐
│              ButtonActorHw<I: InputPin>      │
│                                              │
│  ┌────────────────────────┐                  │
│  │ ButtonActor            │  (generated)     │
│  │ States: Idle →         │                  │
│  │   Debouncing →         │                  │
│  │   PressedState →       │                  │
│  │   Notifying → Released │                  │
│  │   → Idle               │                  │
│  └────────────────────────┘                  │
│                                              │
│  Inputs:  GPIO button pin (InputPin trait)    │
│  Outputs: Channel<ButtonMessage, 4>  (send)  │
│                                              │
│  Rate: 10 ms poll  |  Core: 1 (RP2040)      │
└──────────────────────────────────────────────┘
```

### Maintenance Actor

```
┌─────────────────────────────────────────────┐
│              MaintenanceActorHw<O: OutputPin>│
│                                              │
│  ┌────────────────────────┐                  │
│  │ MaintenanceActor       │  (generated)     │
│  │ States: Idle →         │                  │
│  │   Checking → Toggling  │                  │
│  │   → Reporting → Idle   │                  │
│  └────────────────────────┘                  │
│                                              │
│  Inputs:  (internal tick counter)             │
│  Outputs: GPIO LED pin (OutputPin trait)      │
│           Channel<MaintenanceMessage, 2>     │
│                                              │
│  Rate: 10 Hz  |  Core: 1 (RP2040)           │
└──────────────────────────────────────────────┘
```

## Component Interfaces

| Component | Interface | Direction | Type |
|-----------|-----------|-----------|------|
| ButtonActorHw | `to_control` channel | Out | `Channel<ButtonMessage, 4>` |
| MaintenanceActorHw | `to_control` channel | Out | `Channel<MaintenanceMessage, 2>` |
| ControlActorHw | `from_button` channel | In | `Channel<ButtonMessage, 4>` |
| ControlActorHw | `from_maintenance` channel | In | `Channel<MaintenanceMessage, 2>` |
| ButtonActorHw | `button_pin` | In | `impl InputPin` |
| MaintenanceActorHw | `led` | Out | `impl OutputPin` |
