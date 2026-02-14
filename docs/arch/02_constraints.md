# 2. Constraints

## Technical Constraints

| Constraint | Description |
|-----------|-------------|
| Language | Rust (`no_std`, edition 2024) |
| Async runtime | Embassy (executor, time, sync) |
| MCUs | RP2040 (Cortex-M0+), RP2350 (Cortex-M33), STM32U585 (Cortex-M33) |
| No heap | All allocations are static; `heapless` collections only |
| Linker | `flip-link` for stack overflow protection |
| Logging | `defmt` + RTT (deferred formatting, zero-copy over SWD) |
| Memory | RP2040: 264 KB RAM / 2 MB Flash; RP2350: 520 KB RAM / 4 MB Flash; STM32U585: 768 KB RAM / 2 MB Flash |

## Organizational Constraints

| Constraint | Description |
|-----------|-------------|
| Build system | Cargo + Taskfile (task runner) |
| Dev environment | Nix flake (`flake.nix`) for reproducible toolchains |
| Config management | CUE schema (`data/config/config.cue`) exported to JSON, codegen'd to Rust constants at build time |
| Code generation | SysML 2 models → Rust state machines via `tools/sysml_codegen.py` |
| Formal methods | TLA+ specifications verified with TLC model checker |
| Dependency updates | Renovate bot for automated dependency PRs |

## Conventions

| Convention | Description |
|-----------|-------------|
| Architecture docs | arc42 + C4 model (this document set) |
| Feature flags | One Cargo feature per target: `pico1`, `pico2`, `unoq` |
| Platform entry points | `src/main_<target>.rs` per platform |
| Generated code | `src/generated/` directory, not manually edited |
