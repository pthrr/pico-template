# On-Target Testing with defmt-test

This directory contains integration tests that run on the Raspberry Pi Pico hardware using `defmt-test`.

## Setup

### Prerequisites

1. **Hardware**: Raspberry Pi Pico (1 or 2) connected via USB
2. **Toolchain**:
   ```bash
   rustup target add thumbv6m-none-eabi  # For Pico 1
   rustup target add thumbv8m.main-none-eabihf  # For Pico 2
   ```
3. **Flash Tool**: Either:
   - `picotool` (currently configured)
   - `probe-rs` (recommended by Knurling)

### Installing probe-rs (Recommended)

```bash
cargo install probe-rs-tools --locked
```

Then update `.cargo/config.toml` runner to:
```toml
runner = "probe-rs run --chip RP2040"
```

## Running Tests

### With Hardware Connected

```bash
# For Pico 1
cargo test --test integration --features pico1 --target thumbv6m-none-eabi

# For Pico 2
cargo test --test integration --features pico2 --target thumbv8m.main-none-eabihf
```

### Build Only (No Hardware)

```bash
cargo test --test integration --features pico1 --target thumbv6m-none-eabi --no-run
```

## Test Structure

Tests use the `defmt-test` harness which provides:
- `#[defmt_test::tests]` - Mark test module
- `#[init]` - One-time initialization
- `#[test]` - Individual test functions
- `assert!()` - Standard Rust assertions
- `defmt::info!()` - Logging via RTT

## Example Test

```rust
#[test]
fn test_button_actor_init(_state: &mut ()) {
    defmt::info!("Testing ButtonActor initialization");
    let actor = ButtonActor::new();
    assert!(actor.press_count == 0);
    defmt::info!("Test passed");
}
```

## Sources

- [defmt-test on crates.io](https://crates.io/crates/defmt-test)
- [Testing an embedded application - Ferrous Systems](https://ferrous-systems.com/blog/test-embedded-app/)
- [defmt book](https://defmt.ferrous-systems.com/)
- [probe-rs](https://probe.rs/)
