use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn register_sysml_rerun(manifest_dir: &Path) {
    let model_dir = manifest_dir.join("model");
    if let Ok(entries) = fs::read_dir(model_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "sysml") {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

fn require_generated(manifest_dir: &Path) {
    let generated = manifest_dir.join("src/generated/mod.rs");
    assert!(
        generated.exists(),
        "missing {}; run `task generate` before building firmware",
        generated.display()
    );
}

fn memory_x_src() -> &'static str {
    if cfg!(feature = "unoq") {
        "data/linker/memory-stm32u585.x"
    } else if cfg!(feature = "pico2") {
        "data/linker/memory-pico2.x"
    } else {
        "data/linker/memory-pico1.x"
    }
}

fn platform_id() -> &'static str {
    if cfg!(feature = "unoq") {
        "unoq"
    } else if cfg!(feature = "pico2") {
        "pico2"
    } else {
        "pico1"
    }
}

fn push_json_const(out: &mut String, name: &str, value: &serde_json::Value) {
    match value {
        serde_json::Value::Bool(b) => {
            let _ = writeln!(out, "pub const {name}: bool = {b};");
        }
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                if let Ok(v) = u8::try_from(u) {
                    let _ = writeln!(out, "pub const {name}: u8 = {v};");
                } else if let Ok(v) = u16::try_from(u) {
                    let _ = writeln!(out, "pub const {name}: u16 = {v};");
                } else if let Ok(v) = u32::try_from(u) {
                    let _ = writeln!(out, "pub const {name}: u32 = {v};");
                } else {
                    let _ = writeln!(out, "pub const {name}: u64 = {u};");
                }
            } else if let Some(i) = n.as_i64() {
                if let Ok(v) = i8::try_from(i) {
                    let _ = writeln!(out, "pub const {name}: i8 = {v};");
                } else if let Ok(v) = i16::try_from(i) {
                    let _ = writeln!(out, "pub const {name}: i16 = {v};");
                } else if let Ok(v) = i32::try_from(i) {
                    let _ = writeln!(out, "pub const {name}: i32 = {v};");
                } else {
                    let _ = writeln!(out, "pub const {name}: i64 = {i};");
                }
            } else if let Some(f) = n.as_f64() {
                let _ = writeln!(out, "pub const {name}: f64 = {f};");
            }
        }
        serde_json::Value::String(s) => {
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            let _ = writeln!(out, "pub const {name}: &str = \"{escaped}\";");
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            let json_str = serde_json::to_string(value).expect("serialize JSON value");
            let _ = writeln!(out, "pub const {name}: &str = r#\"{json_str}\"#;");
        }
        serde_json::Value::Null => {
            let _ = writeln!(out, "pub const {name}: &str = \"\";");
        }
    }
}

fn write_config_rs(out_dir: &Path, platform: &str) {
    let cue_expr = format!("#Platform.{platform}");
    let output = Command::new("cue")
        .args(["export", "data/config/config.cue", "-e", &cue_expr])
        .output()
        .expect("execute cue");

    assert!(
        output.status.success(),
        "CUE export failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::write(out_dir.join("config.json"), &output.stdout).expect("write config.json");

    let json_str = String::from_utf8(output.stdout).expect("UTF-8 from cue");
    let config: serde_json::Value = serde_json::from_str(&json_str).expect("parse CUE JSON");

    let mut config_code = String::from("// Auto-generated from CUE config\n\n");
    if let Some(obj) = config.as_object() {
        for (key, value) in obj {
            push_json_const(&mut config_code, &key.to_uppercase(), value);
        }
    }

    fs::write(out_dir.join("config.rs"), config_code).expect("write config.rs");
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    register_sysml_rerun(&manifest_dir);
    require_generated(&manifest_dir);

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let out_dir = Path::new(&out_dir);

    fs::copy(memory_x_src(), out_dir.join("memory.x")).expect("copy memory.x");

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/linker/memory-pico1.x");
    println!("cargo:rerun-if-changed=data/linker/memory-pico2.x");
    println!("cargo:rerun-if-changed=data/linker/memory-stm32u585.x");
    println!("cargo:rerun-if-changed=data/config/config.cue");

    write_config_rs(out_dir, platform_id());
}
