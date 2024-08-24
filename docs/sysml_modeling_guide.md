# SysML v2 Modeling Guide

Guide for writing SysML v2 actor models that feed into the codegen pipeline (`tools/sysml_codegen/`). The codegen branches each model into two artifact families:

- **Rust** (`src/generated/mod.rs` + sibling `*.rs` per `pub mod`) — multi-file module layout. Single-file crate modules stay as `src/<name>.rs`. SysML is the single source of truth for actor cadence.
- **mCRL2** (`model/mcrl2/*.mcrl2` + `.mcf`) — process-algebraic specs with deadlock freedom, liveness, and timed deadline properties; plus a composed system spec for inter-actor channels.

## Model Structure

Each actor lives in its own `.sysml` file under `model/`. A file is a package containing one `part def` (the actor), plus supporting definitions:

```sysml
package MyPackage {
    private import ScalarValues::*;

    // 1. Port definitions (optional, for composed systems)
    // 2. Action definitions (reusable logic blocks)
    // 3. State definitions (with entry/do/exit actions)
    // 4. Part definition (the actor itself)
}
```

The system composition lives in a separate file (`system.sysml`) that wires actors together via connections.

## Attributes

```sysml
attribute name : Type = default_value;
```

Supported types and their mappings:

| SysML Type | Rust | mCRL2 | Notes |
|------------|------|-------|-------|
| `Boolean` | `bool` | `Bool` | |
| `Integer` | `u32`/`u16`/`u8` | `Nat` | Must be non-negative |
| `Real` | `f32` | `Int` | Truncated to integer in the formal model |

Every attribute **must have a default value** — it becomes the initial state in the generated model and the initial process parameters in mCRL2.

Comma-separated names share a type and default:

```sysml
attribute x, y, z : Integer = 0;
```

## Action Definitions

Defined at package level, referenced by states:

```sysml
action def reset {
    counter := 0;
    flag := false;
}
```

**Assignment syntax:** `:=` (not `=`).

**Supported expressions:**
- Arithmetic: `+`, `-`, `*`, `mod`
- Logic: `not`, `and`, `or`
- Comparison: `>=`, `<=`, `>`, `<`, `=`
- Grouping: parentheses

**`mod` for bounding variables** — see [Bounding Variables](#bounding-variables) below.

## State Definitions

Defined at package level, referenced in the state machine via `state : Name;`:

```sysml
state def Running {
    doc /*"Normal operation"*/
    entry action : initialize;
    do action : count;
    exit action : cleanup;
}
```

- `entry` — runs once on entering the state
- `do` — continuous effect while in the state (generates assignments on every step in mCRL2)
- `exit` — runs once on leaving the state
- All three are optional
- `doc /*"..."*/` — optional documentation string

## State Machine

Defined inside the `part def` using the exhibited pattern:

```sysml
part def MyActor {
    // attributes...

    exhibit state machine {
        state : Idle;
        state : Running;
        state : Error;

        transition first Idle if ready then Running;
        transition first Running then Idle;
    }
}
```

## Transitions

### Unconditional

```sysml
transition first FromState then ToState;
```

Always fires when in `FromState`.

### With guard condition

```sysml
transition first FromState if condition then ToState;
```

Only fires when `condition` is true. The condition references the actor's own attributes.

### With accept (environment input)

```sysml
transition first FromState accept variable then ToState;
```

The `accept` keyword marks `variable` as an **environment input** — something the actor does not control. In mCRL2, this becomes a nondeterministic `sum` over the variable's domain. Use `accept` for:
- External signals (button presses, error flags from hardware)
- Inputs received from other actors

Do **not** use `accept` for internal state variables like counters or flags the actor sets itself.

### Accept with guard

```sysml
transition first FromState accept env_var if guard then ToState;
```

`env_var` is the environment input, `guard` is an additional condition on internal state. The guard is **not** treated as an environment input.

### With transition action

```sysml
transition first FromState do action : action_name then ToState;
```

The action executes during the transition (after exit actions of the source state, before entry actions of the target state).

### With label

```sysml
transition my_label first FromState if cond then ToState;
```

Labels are optional and ignored by the codegen. Useful for documentation.

### Multi-line transitions

Transitions can span multiple lines — the parser collects everything between `transition` and `;`:

```sysml
transition init_to_run
    first Initializing
    if cycle_count > 100
    then Running;
```

## Transition Priority

When multiple transitions leave the same state, **order matters**. The codegen generates them in the listed order. For mCRL2, earlier transitions get priority; later transitions' guards are implicitly negated in self-loop generation.

List error/safety transitions **first**:

```sysml
// Error takes priority
transition first Running accept error_flag then Error;
// Normal operation
transition first Running if enabled then Processing;
// Watchdog fallback
transition first Running if cycle_count > limit then Initializing;
```

## Bounding Variables

For **exact proofs** (complete state space exploration rather than bounded checking), all integer variables that can grow must be bounded using `mod`:

```sysml
attribute max_count : Integer = 255;

action def increment {
    count := (count + 1) mod (max_count + 1);
}
```

This wraps `count` to `[0, max_count]`, keeping the state space finite.

**Rules:**
- `mod` maps to mCRL2's `mod` on `Nat` — the right operand must be positive, so use `mod (limit + 1)` where `limit >= 0`
- Without bounding, the state space is infinite and verification can only explore up to `MAX_STATES` (default: 1,000,000)
- Bounded models get exact deadlock freedom and liveness proofs

### Watchdog Pattern

If a state has a `do action` that increments a counter, the counter can grow unboundedly, preventing liveness (the system can stay in that state forever). Add a watchdog to force a transition:

```sysml
attribute watchdog_limit : Integer = 1000;

// Error: guarded by watchdog
transition first Running accept error_flag if cycle_count <= watchdog_limit then Error;

// Normal: guarded by watchdog
transition first Running if enabled and cycle_count <= watchdog_limit then Processing;

// Watchdog forces reinitialization
transition first Running if cycle_count > watchdog_limit then Initializing;
```

This guarantees the actor eventually leaves `Running`, satisfying liveness.

## Ports and Signals

Ports define the interface for inter-actor communication:

```sysml
port def ButtonPort {
    out signal pressed_event : Boolean;
    out signal released_event : Boolean;
}
```

- `out signal` — the actor sends this
- `in signal` — the actor receives this
- Signals can have types: `Boolean`, `Integer`, or array types like `Integer[4]`

Reference ports in the actor:

```sysml
part def ButtonActor {
    port button_out : ButtonPort;        // sends signals
    // ...
}

part def ControlActor {
    port button_in : ~ButtonPort;        // conjugated: receives signals
    // ...
}
```

The `~` prefix conjugates the port (flips in/out directions).

## System Composition

The system file wires actors together:

```sysml
package System {
    private import ScalarValues::*;

    part def PicoSystem {
        part button : ButtonActor;
        part control : RealtimeControlActor;
        part maintenance : MaintenanceActor;

        connection button.button_out to control.button_in, 4;
        connection maintenance.status_out to control.maintenance_in, 2;
    }
}
```

- `part name : ActorType;` — actor instances
- `connection from.port to to.port, capacity;` — channel with buffer capacity (default: 2 if omitted)
- The codegen generates a composed mCRL2 spec with buffer processes, parallel composition, and `allow`/`comm` operators
- It also generates `src/generated/channels.rs` (`pub static` Embassy channels; payload types from `src/messages.rs`)

## Requirements

Documentary constraints — not verified by the codegen, but captured in the model:

```sysml
requirement {
    subject;
    doc /* Description */
    require constraint {
        expression
    }
}
```

## WCET Annotations

Action definitions can include a `wcet N;` annotation specifying the worst-case execution time in microseconds:

```sysml
action def initialize {
    wcet 200;
    cycle_count := 0;
    control_value := 0.0;
}
```

The `wcet` line must be inside the action def body. The value is in microseconds. The annotation is optional — actions without `wcet` are treated as instantaneous in the timed model.

### Phase-Based Timed Model

When any action in an actor has a WCET annotation, the timed mCRL2 spec uses a **phase-based model** instead of the default instantaneous-step model. The actor process gains an additional `phase: Nat` parameter:

- **`phase > 0`**: The actor is "computing" — it can only `tick` (decrementing phase each tick)
- **`phase == 0`**: The actor is idle — state transitions can fire
- **Entering a state** sets `phase = PHASE_<StateName>` where the phase cost is the ceiling division of the sum of entry + do action WCETs by the time step

This models the fact that actions take time proportional to their WCET. For periodic actors, a `deadline_miss` action fires if `phase > 0` when the period expires — meaning the actor didn't finish its computation before the next activation.

### Generated Properties

When WCET annotations are present:

- **`*_timed_deadline.mcf`** (periodic actors only): No deadline misses — `[true*][deadline_miss]false`
- **`*_timed_deadlock_freedom.mcf`**: Unchanged — `[true*]<true>true`
- **`*_timed_response.mcf`**: Unchanged — after activation/input, a step eventually occurs

### Schedulability Check

At codegen time, actors are grouped by their `core` attribute. For each core, the sum of worst-case WCETs is compared against the shortest period. A warning is emitted if `sum >= period`, indicating potential deadline misses under worst-case interleaving. This is a necessary (conservative) condition for Embassy's cooperative scheduling model.

### Graceful Fallback

If no actions have WCET annotations, the existing timed model (without `phase`) is used unchanged. No unnecessary state space expansion occurs.

## Timed mCRL2 Models

Actors with timing attributes (`execution_period_ms`, `max_execution_time_us`, `max_jitter_us`, `debounce_period_ms`) automatically get **timed mCRL2 specs** in addition to the untimed ones. Timed specs use discrete tick-based time with a `Nat`-valued `elapsed` process parameter.

### How It Works

The codegen computes a time step as the GCD of all timing values (in µs), then converts everything to tick counts:

| Constant | Formula |
|----------|---------|
| `PERIOD_TICKS` | `execution_period_ms × 1000 / TIME_STEP` |
| `DEADLINE_TICKS` | `max_execution_time_us / TIME_STEP` |
| `JITTER_TICKS` | `max_jitter_us / TIME_STEP` |
| `DEBOUNCE_TICKS` | `debounce_period_ms × 1000 / TIME_STEP` |

### Two Actor Patterns

**Periodic actors** (have `execution_period_ms`): get `activate` action for periodic reactivation within the jitter window, `tick` for time advance, and `deadline_miss` if work isn't completed within `DEADLINE_TICKS`.

**Event-driven actors** (have `debounce_period_ms`, no period): get debounce enforcement on `env_*` inputs — the actor only accepts new events after `DEBOUNCE_TICKS` have elapsed.

### Generated Properties

- `*_timed_deadline.mcf` — No deadline misses: `[true*][deadline_miss]false`
- `*_timed_response.mcf` — After activation/input, a step eventually occurs before the next activation

### Which Actors Get Timed Specs

| Actor | Timed? | Pattern |
|-------|--------|---------|
| `RealtimeControlActor` | Yes | Periodic (1ms period, 800µs deadline, 50µs jitter) |
| `MaintenanceActor` | Yes | Periodic (100ms period, 5000µs deadline) |
| `ButtonActor` | Yes | Event-driven (10ms debounce, 100µs deadline) |

### Output Files

```
model/mcrl2/
    ├─→ RealtimeControlActor_timed.mcrl2
    ├─→ RealtimeControlActor_timed_deadline.mcf
    ├─→ RealtimeControlActor_timed_response.mcf
    ├─→ MaintenanceActor_timed.mcrl2
    ├─→ ...
    └─→ ButtonActor_timed.mcrl2
```

## Codegen Pipeline

```
model/*.sysml
    │
    ├─→ src/generated/mod.rs            (pub mod button; …)
    ├─→ src/generated/<actor>.rs        (state machine + timing/WCET consts)
    ├─→ src/generated/channels.rs       (Embassy pub static channels)
    ├─→ src/generated/tasks.rs          (periodic / debounce scheduling loops)
    │
    └─→ model/mcrl2/*.mcrl2              (mCRL2 specs; gitignored, regenerate locally)
              ├─→ *_deadlock_freedom.mcf
              ├─→ *_liveness.mcf
              ├─→ *_timed.mcrl2          (timed specs)
              ├─→ *_timed_deadline.mcf
              └─→ *_timed_response.mcf
```

Run with:

```sh
task generate            # SysML → src/generated + model/mcrl2 (both gitignored; also run from firmware build.rs)
task codegen:check       # type-check the codegen tool
task verify_mcrl2        # generate, then bounded mCRL2 checks (parallel per spec)
task verify_timed_mcrl2
# Optional: JOBS=8 MAX_STATES=500000 PBES_TIMEOUT=120 task verify_mcrl2
```

### SysML attribute → Rust constant mapping

| SysML attribute | Rust constant | Unit conversion |
|-----------------|---------------|-----------------|
| `execution_period_ms : Integer` | `Self::EXECUTION_PERIOD_US: u64` | × 1000 |
| `debounce_period_ms : Integer` | `Self::DEBOUNCE_PERIOD_US: u64` | × 1000 |
| `max_jitter_us : Integer` | `Self::MAX_JITTER_US: u64` | — |
| `max_execution_time_us : Integer` | `Self::MAX_EXECUTION_TIME_US: u64` | — |
| `priority : Integer` | `Self::PRIORITY: u8` | — |
| `core : Integer` | `Self::CORE: u8` | — |
| `wcet N` inside `action def` | `Self::WCET_<STATE>_US: u64` (sum per state) | — |

`src/generated/tasks.rs` references these constants (e.g. `RealtimeControlActor::EXECUTION_PERIOD_US`); `src/tasks.rs` only passes `|| actor.step()` into the generated loop. Changing a SysML timing attribute regenerates mCRL2, constants, and the scheduling loop.

## Common Pitfalls

**Unbounded integers** — Without `mod`, integer variables create infinite state spaces. Verification can only do bounded exploration (up to `MAX_STATES`). Bound all counters.

**`accept` on internal variables** — The variable after `accept` becomes a nondeterministic environment input (`sum` in mCRL2). If you use `accept` on an internal counter, it will be treated as externally controlled, which is wrong.

**Unreachable or deadlocked states** — Every state must have at least one outgoing transition path. The codegen auto-generates self-loops with negated guards, but if your guard logic is contradictory you can still get deadlocks.

**`Real` type precision** — `Real` maps to `Int` in mCRL2 (not floating-point). Use it only for values that are effectively integers in the formal model. The default `0.0` becomes `0`.

**`mod` right operand** — Must be positive (`Pos` type in mCRL2). `mod 0` will fail. Always use `mod (limit + 1)` where `limit >= 0`.

**Transition order** — List error/priority transitions before normal ones. The codegen respects the listed order for guard precedence.

## Complete Example

A minimal actor with bounded state space:

```sysml
package Counter {
    private import ScalarValues::*;

    action def reset {
        count := 0;
    }

    action def increment {
        count := (count + 1) mod (max_count + 1);
    }

    state def Idle {
        doc /*"Waiting"*/
        entry action : reset;
    }

    state def Counting {
        doc /*"Counting events"*/
        do action : increment;
    }

    part def CounterActor {
        attribute count : Integer = 0;
        attribute max_count : Integer = 255;
        attribute active : Boolean = false;

        attribute priority : Integer = 5;
        attribute core : Integer = 1;

        exhibit state machine {
            state : Idle;
            state : Counting;

            transition first Idle accept active then Counting;
            transition first Counting if count >= max_count then Idle;
        }
    }
}
```
