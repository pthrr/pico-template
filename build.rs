use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Select linker script based on feature flags
    let memory_x_src = if cfg!(feature = "pico2") {
        "data/linker/memory-pico2.x"
    } else {
        "data/linker/memory-pico1.x"
    };

    // Copy selected memory.x to output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir_path = Path::new(&out_dir);
    fs::copy(memory_x_src, out_dir_path.join("memory.x")).expect("Failed to copy memory.x");

    println!("cargo:rustc-link-search={}", out_dir);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/linker/memory-pico1.x");
    println!("cargo:rerun-if-changed=data/linker/memory-pico2.x");
    println!("cargo:rerun-if-changed=data/config/config.cue");

    // Export CUE config to JSON
    let output = Command::new("cue")
        .args(["export", "data/config/config.cue", "-e", "selected"])
        .output()
        .expect("Failed to execute cue");

    if !output.status.success() {
        panic!(
            "CUE export failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let json_str = String::from_utf8(output.stdout.clone()).expect("Invalid UTF-8 from cue");
    let config: serde_json::Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");

    // Write JSON to output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let json_path = Path::new(&out_dir).join("config.json");
    fs::write(&json_path, &output.stdout).expect("Failed to write config.json");

    // Generate Rust constants from JSON
    let config_rs_path = Path::new(&out_dir).join("config.rs");
    let mut config_code = String::from("// Auto-generated from CUE config\n\n");

    if let Some(obj) = config.as_object() {
        for (key, value) in obj {
            let const_name = key.to_uppercase();
            match value {
                serde_json::Value::Bool(b) => {
                    config_code.push_str(&format!("pub const {}: bool = {};\n", const_name, b));
                }
                serde_json::Value::Number(n) => {
                    if let Some(u) = n.as_u64() {
                        if u <= u8::MAX as u64 {
                            config_code
                                .push_str(&format!("pub const {}: u8 = {};\n", const_name, u));
                        } else if u <= u16::MAX as u64 {
                            config_code
                                .push_str(&format!("pub const {}: u16 = {};\n", const_name, u));
                        } else if u <= u32::MAX as u64 {
                            config_code
                                .push_str(&format!("pub const {}: u32 = {};\n", const_name, u));
                        } else {
                            config_code
                                .push_str(&format!("pub const {}: u64 = {};\n", const_name, u));
                        }
                    } else if let Some(i) = n.as_i64() {
                        if i >= i8::MIN as i64 && i <= i8::MAX as i64 {
                            config_code
                                .push_str(&format!("pub const {}: i8 = {};\n", const_name, i));
                        } else if i >= i16::MIN as i64 && i <= i16::MAX as i64 {
                            config_code
                                .push_str(&format!("pub const {}: i16 = {};\n", const_name, i));
                        } else if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                            config_code
                                .push_str(&format!("pub const {}: i32 = {};\n", const_name, i));
                        } else {
                            config_code
                                .push_str(&format!("pub const {}: i64 = {};\n", const_name, i));
                        }
                    } else if let Some(f) = n.as_f64() {
                        config_code.push_str(&format!("pub const {}: f64 = {};\n", const_name, f));
                    }
                }
                serde_json::Value::String(s) => {
                    config_code.push_str(&format!(
                        "pub const {}: &str = \"{}\";\n",
                        const_name,
                        s.replace("\\", "\\\\").replace("\"", "\\\"")
                    ));
                }
                serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                    // Serialize arrays and objects as JSON strings
                    let json_str = serde_json::to_string(value).unwrap();
                    config_code.push_str(&format!(
                        "pub const {}: &str = r#\"{}\"#;\n",
                        const_name, json_str
                    ));
                }
                serde_json::Value::Null => {
                    // Represent null as Option<&str> = None, but for const we use empty string
                    config_code.push_str(&format!("pub const {}: &str = \"\";\n", const_name));
                }
            }
        }
    }

    fs::write(&config_rs_path, config_code).expect("Failed to write config.rs");
}
