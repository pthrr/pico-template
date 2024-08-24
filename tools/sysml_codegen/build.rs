//! For test builds only: pass `GENERATED_DIR` / `MODEL_DIR` into `env!()` in tests.
//! Bin (`cargo run`) does not need these; `task test` sets them in `tools/Taskfile.yml`.

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GENERATED_DIR");
    println!("cargo:rerun-if-env-changed=MODEL_DIR");

    if let Ok(generated) = env::var("GENERATED_DIR") {
        println!("cargo:rerun-if-changed={generated}");
        println!("cargo:rustc-env=GENERATED_DIR={generated}");
    }
    if let Ok(model) = env::var("MODEL_DIR") {
        println!("cargo:rerun-if-changed={model}");
        println!("cargo:rustc-env=MODEL_DIR={model}");
    }
}
