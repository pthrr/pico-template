# 11. Risks and Technical Debt

## Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| SysML/TLA+ model drift | Medium | High | Currently manual sync; could automate SysML → TLA+ generation |
| Embassy breaking changes | Medium | Medium | Pin dependency versions; Renovate bot for controlled updates |
| `cue` CLI not available at build time | Low | High | `build.rs` panics if `cue export` fails; document in dev setup |
| Core 1 stack overflow (4 KB fixed) | Low | High | `flip-link` catches overflow as HardFault; monitor with defmt |
| Channel message drop under overload | Medium | Low | `try_send` is non-blocking by design; acceptable for button/maintenance messages |
| UNO Q platform parity | Medium | Low | UNO Q currently only has blink demo, not full actor system |

## Technical Debt

| Item | Description | Priority |
|------|-------------|----------|
| UNO Q actor integration | `main_unoq.rs` only has a blink task, not the full actor system (control, button, maintenance) | Medium |
| CUE config still references `stm32u585` and `uno_q` | `data/config/config.cue` has stale platform names (`stm32u585`, `uno_q` instead of `unoq`) | Low |
| `flip-link` build dependency warning | `Cargo.toml` lists `flip-link` as a build dependency but it has no lib target; causes cargo warning | Low |
| No host-side unit tests | All tests require on-target execution via `defmt_test`; no `#[cfg(test)]` host tests for generated state machines | Medium |
| Generated `.rlib` in source tree | `src/generated/libbutton.rlib` is a compiled artifact checked into source | Low |
