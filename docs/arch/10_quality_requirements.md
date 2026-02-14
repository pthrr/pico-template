# 10. Quality Requirements

## Quality Tree

```
                    Quality
                   /   |   \
                  /    |    \
        Reliability  Performance  Maintainability
            |            |              |
       Correctness   Determinism    Portability
       Safety        Efficiency     Traceability
```

## Quality Scenarios

| ID | Quality Attribute | Scenario | Metric | Priority |
|----|-------------------|----------|--------|----------|
| QS1 | Determinism | Control loop executes every 1 ms under normal load | Jitter < 50 us, execution time < 800 us | High |
| QS2 | Correctness | Button press is detected and debounced | Stable signal for 5 cycles (50 ms) before reporting press | High |
| QS3 | Correctness | Actor state machines have no unreachable or deadlock states | TLA+ model checking passes with no counterexamples | High |
| QS4 | Portability | Adding a new MCU target requires only platform-layer changes | No modifications to `tasks.rs`, `*_hw.rs`, `messages.rs`, or `generated/` | Medium |
| QS5 | Reliability | LED heartbeat indicates system liveness | LED toggles every 1 second (10 maintenance cycles at 10 Hz) | Medium |
| QS6 | Efficiency | Firmware fits in flash and runs within RAM constraints | Binary < 2 MB, RAM usage < 264 KB (smallest target) | Medium |
| QS7 | Traceability | Actor behavior traces from model to running code | SysML → codegen → Rust; SysML → TLA+ → verification | Medium |
| QS8 | Reliability | Stack overflow causes a deterministic fault, not silent corruption | `flip-link` inverts memory layout; overflow triggers HardFault | Medium |
