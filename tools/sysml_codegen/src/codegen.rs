//! Rust code generation from SysML AST.
//!
//! Thin wrapper: translate to typed IR, then render to string.

use crate::ast::{Package, PartDef};
use crate::render;
use crate::tla_render;
use crate::translate;
use std::collections::HashMap;

/// Generate a complete Rust source file from a SysML package.
pub fn generate(package: &Package) -> String {
    let module = translate::translate(package);
    render::render(&module)
}

/// Generate TLA+ specifications from a SysML package.
/// Returns `(part_name, tla_content, cfg_content)` per part with a state machine.
pub fn generate_tla(package: &Package) -> Vec<(String, String, String)> {
    tla_render::render_tla(package)
}

/// Generate channel declarations for a system part def.
/// Message types are generated in per-actor files alongside their port defs.
pub fn generate_system_channels(
    system: &PartDef,
    parts: &HashMap<String, &PartDef>,
) -> String {
    if system.connections.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    // define_channels! from connections.
    // connection button.button_out to control.button_in, 4
    //   → BUTTON_TO_CONTROL: ButtonMessage, 4;
    // Message type derived from source port type: ButtonPort → ButtonMessage
    out.push_str("define_channels! {\n");
    for conn in &system.connections {
        let chan_name = format!("{}_TO_{}",
            conn.from_part.to_uppercase(),
            conn.to_part.to_uppercase());
        let msg_name = find_port_message_name(
            &conn.from_part, &conn.from_port, parts,
        );
        out.push_str(&format!("    {chan_name}: {msg_name}, {};\n", conn.capacity));
    }
    out.push_str("}\n");

    out
}

/// Look up a port's type on a part instance and derive the message struct name.
fn find_port_message_name(
    part_name: &str,
    port_name: &str,
    parts: &HashMap<String, &PartDef>,
) -> String {
    if let Some(part_def) = parts.get(part_name) {
        if let Some(port) = part_def.ports.iter().find(|p| p.name == port_name) {
            return port_type_to_message_name(&port.typ);
        }
    }
    // Fallback: derive from part instance name
    format!("{}Message", to_pascal_case(part_name))
}

/// `ButtonPort` → `ButtonMessage`, `StatusPort` → `StatusMessage`.
fn port_type_to_message_name(port_type: &str) -> String {
    if let Some(base) = port_type.strip_suffix("Port") {
        format!("{base}Message")
    } else {
        format!("{port_type}Message")
    }
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}
