//! Rust code generation from `SysML` AST.
//!
//! Thin wrapper: translate to typed IR, then render to string.

use crate::ast::{Package, PartDef};
use crate::buf;
use crate::mcrl2_render;
use crate::render;
use crate::translate;
use std::collections::HashMap;
use std::path::Path;

/// Generate a complete Rust source file from a `SysML` package.
pub fn generate(package: &Package) -> String {
    let module = translate::translate(package);
    let mut code = render::render(&module);
    code.push_str(&generate_actor_constants(package));
    code
}

/// Generate mCRL2 specifications from a `SysML` package.
/// Returns `(part_name, mcrl2_content, [(prop_name, mcf_content)])` per part with a state machine.
pub fn generate_mcrl2(package: &Package) -> Vec<mcrl2_render::Mcrl2PartOutput> {
    mcrl2_render::render_mcrl2(package)
}

/// Generate timed mCRL2 specifications from a `SysML` package.
pub fn generate_timed_mcrl2(package: &Package) -> Vec<mcrl2_render::Mcrl2PartOutput> {
    mcrl2_render::render_timed_mcrl2(package)
}

/// Generate the `src/generated/channels/mod.rs` body from a system composition.
///
/// Emits raw `pub static` Embassy `Channel`s — one per connection — so that
/// platform-specific `main_*.rs` files can reference them via
/// `pico_template::generated::channels::<NAME>` without invoking any helper
/// macro. The message struct/enum types are expected to live in
/// `crate::messages` (hand-written, since `SysML` port signals don't capture
/// the runtime payload shape for this template).
///
/// Naming convention (symmetric with `mcrl2_compose`):
///   connection `<from>.<port_a> to <to>.<port_b>, N`
///     ⇒ `pub static <FROM>_TO_<TO>: Channel<…, <From>Message, N>`
pub fn generate_system_channels(system: &PartDef, _parts: &HashMap<String, &PartDef>) -> String {
    let mut out = String::new();
    out.push_str("//! Auto-generated channel topology from SysML system composition.\n");
    out.push_str("//! Each connection in `model/system.sysml` becomes one `pub static` Embassy\n");
    out.push_str("//! `Channel`. The payload types come from `crate::messages`.\n\n");

    if system.connections.is_empty() {
        out.push_str("// (no inter-actor connections defined)\n");
        return out;
    }

    // Collect distinct message types so we can emit one `use` statement.
    let mut msg_types: Vec<String> = Vec::new();
    for conn in &system.connections {
        let m = source_message_type(&conn.from_part);
        if !msg_types.contains(&m) {
            msg_types.push(m);
        }
    }
    buf::append(
        &mut out,
        format_args!("use crate::messages::{{{}}};\n", msg_types.join(", ")),
    );
    out.push_str("use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;\n");
    out.push_str("use embassy_sync::channel::Channel;\n\n");

    for conn in &system.connections {
        let chan_name = format!(
            "{}_TO_{}",
            conn.from_part.to_uppercase(),
            conn.to_part.to_uppercase()
        );
        let msg_name = source_message_type(&conn.from_part);
        buf::append(
            &mut out,
            format_args!(
                "pub static {chan_name}: Channel<CriticalSectionRawMutex, {msg_name}, {}> =\n    Channel::new();\n",
                conn.capacity
            ),
        );
    }

    out
}

/// Derive the runtime message type for a connection's source actor.
/// `button` → `ButtonMessage`, `maintenance` → `MaintenanceMessage`.
fn source_message_type(part_instance: &str) -> String {
    format!("{}Message", to_pascal_case(part_instance))
}

/// Remove prior generated Rust under `output_dir` (stale flat `*.rs` or `<name>/` dirs).
pub fn clear_generated_rust(output_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(output_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Write `mod.rs` listing all generated submodules (crate root for `src/generated/`).
pub fn write_mod_rs(output_dir: &Path, modules: &[String]) {
    let mut mods: Vec<_> = modules.to_vec();
    mods.sort();
    mods.dedup();

    let mut out = String::from("//! Auto-generated from SysML models.\n\n");
    for name in &mods {
        buf::append(&mut out, format_args!("pub mod {name};\n"));
    }
    std::fs::write(output_dir.join("mod.rs"), out).expect("write mod.rs");
}

/// Generate the `tasks` module body (written to `tasks/mod.rs`).
///
/// Scheduling pattern is chosen from the `SysML` attribute set:
///   * `execution_period_ms` ⇒ strict periodic loop (`target_period` − elapsed).
///   * `debounce_period_ms`  ⇒ pre-sleep loop (sleep, then step).
///
/// Each helper takes a `FnMut()` so the caller owns the actor + hardware and
/// the generated code stays free of platform types.
pub fn generate_task_loops(packages: &[Package]) -> String {
    let mut out = String::new();
    out.push_str("//! Auto-generated Embassy scheduling loops, one per timed `SysML` actor.\n");
    out.push_str("//!\n");
    out.push_str("//! Each helper runs the actor's `step()` (passed as a closure) at the period\n");
    out.push_str(
        "//! and deadline declared in `model/*.sysml`. WCET overruns and missed deadlines\n",
    );
    out.push_str("//! are logged via `defmt`; no panic, no silent drop.\n\n");

    let mut entries: Vec<TaskEntry> = Vec::new();
    for pkg in packages {
        let module = pkg.name.to_lowercase();
        for part in &pkg.parts {
            if part.state_machine.is_none() {
                continue;
            }
            let period_us = attr_u64(part, "execution_period_ms").map(|ms| ms * 1000);
            let debounce_us = attr_u64(part, "debounce_period_ms").map(|ms| ms * 1000);
            if period_us.is_none() && debounce_us.is_none() {
                continue;
            }
            entries.push(TaskEntry {
                module: module.clone(),
                actor: part.name.clone(),
                fn_name: actor_loop_name(&part.name),
                kind: if period_us.is_some() {
                    LoopKind::Periodic
                } else {
                    LoopKind::Debounce
                },
            });
        }
    }

    if entries.is_empty() {
        out.push_str("// (no timed actors in the SysML model)\n");
        return out;
    }

    out.push_str("use embassy_time::{Duration, Instant, Timer};\n\n");
    for e in &entries {
        buf::append(
            &mut out,
            format_args!("use crate::generated::{}::{};\n", e.module, e.actor),
        );
    }
    out.push('\n');

    for e in &entries {
        match e.kind {
            LoopKind::Periodic => render_periodic_loop(&mut out, e),
            LoopKind::Debounce => render_debounce_loop(&mut out, e),
        }
    }

    out
}

struct TaskEntry {
    module: String,
    actor: String,
    fn_name: String,
    kind: LoopKind,
}

#[derive(Clone, Copy)]
enum LoopKind {
    Periodic,
    Debounce,
}

/// `ButtonActor` → `button_loop`, `RealtimeControlActor` → `realtime_control_loop`.
fn actor_loop_name(actor: &str) -> String {
    let base = actor.strip_suffix("Actor").unwrap_or(actor);
    format!("{}_loop", pascal_to_snake(base))
}

fn pascal_to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn render_periodic_loop(out: &mut String, e: &TaskEntry) {
    let TaskEntry { actor, fn_name, .. } = e;
    buf::append(
        out,
        format_args!("/// Strict-period scheduling loop for `{actor}`.\n"),
    );
    out.push_str("/// Period and deadline come from `SysML` attributes on the actor.\n");
    buf::append(
        out,
        format_args!("pub async fn {fn_name}<F: FnMut()>(mut step: F) {{\n"),
    );
    buf::append(
        out,
        format_args!("    const PERIOD_US: u64 = {actor}::EXECUTION_PERIOD_US;\n"),
    );
    buf::append(
        out,
        format_args!("    const WCET_US: u64 = {actor}::WCET_MAX_US;\n"),
    );
    out.push_str("    let period = Duration::from_micros(PERIOD_US);\n");
    out.push_str("    defmt::info!(\n");
    buf::append(
        out,
        format_args!("        \"{actor}: period={{}}us wcet_max={{}}us\",\n"),
    );
    out.push_str("        PERIOD_US, WCET_US,\n");
    out.push_str("    );\n");
    out.push_str("    loop {\n");
    out.push_str("        let t0 = Instant::now();\n");
    out.push_str("        step();\n");
    out.push_str("        let elapsed = Instant::now() - t0;\n");
    out.push_str("        let elapsed_us = elapsed.as_micros();\n");
    out.push_str("        if elapsed_us > WCET_US {\n");
    out.push_str("            defmt::warn!(\n");
    buf::append(
        out,
        format_args!("                \"{actor}: WCET overrun {{}}us > {{}}us\",\n"),
    );
    out.push_str("                elapsed_us, WCET_US,\n");
    out.push_str("            );\n");
    out.push_str("        }\n");
    out.push_str("        if elapsed < period {\n");
    out.push_str("            Timer::after(period - elapsed).await;\n");
    out.push_str("        } else {\n");
    out.push_str("            defmt::info!(\n");
    buf::append(
        out,
        format_args!("                \"{actor}: missed deadline by {{}}us\",\n"),
    );
    out.push_str("                (elapsed - period).as_micros(),\n");
    out.push_str("            );\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
}

fn render_debounce_loop(out: &mut String, e: &TaskEntry) {
    let TaskEntry { actor, fn_name, .. } = e;
    buf::append(
        out,
        format_args!("/// Debounce loop for `{actor}`: sleep first, then `step()`.\n"),
    );
    out.push_str("/// Period from `SysML` `debounce_period_ms`; deadline from `WCET_MAX_US`.\n");
    buf::append(
        out,
        format_args!("pub async fn {fn_name}<F: FnMut()>(mut step: F) {{\n"),
    );
    buf::append(
        out,
        format_args!("    const DEBOUNCE_US: u64 = {actor}::DEBOUNCE_PERIOD_US;\n"),
    );
    buf::append(
        out,
        format_args!("    const WCET_US: u64 = {actor}::WCET_MAX_US;\n"),
    );
    out.push_str("    defmt::info!(\n");
    buf::append(
        out,
        format_args!("        \"{actor}: debounce={{}}us wcet_max={{}}us\",\n"),
    );
    out.push_str("        DEBOUNCE_US, WCET_US,\n");
    out.push_str("    );\n");
    out.push_str("    loop {\n");
    out.push_str("        Timer::after(Duration::from_micros(DEBOUNCE_US)).await;\n");
    out.push_str("        let t0 = Instant::now();\n");
    out.push_str("        step();\n");
    out.push_str("        let elapsed_us = (Instant::now() - t0).as_micros();\n");
    out.push_str("        if elapsed_us > WCET_US {\n");
    out.push_str("            defmt::warn!(\n");
    buf::append(
        out,
        format_args!("                \"{actor}: WCET overrun {{}}us > {{}}us\",\n"),
    );
    out.push_str("                elapsed_us, WCET_US,\n");
    out.push_str("            );\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
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

/// Convert `PascalCase` to `SCREAMING_SNAKE_CASE`.
/// `"Initializing"` → `"INITIALIZING"`, `"PressedState"` → `"PRESSED_STATE"`.
fn to_screaming_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

/// Look up an attribute's default value, parsed as a `u64`.
fn attr_u64(part: &PartDef, name: &str) -> Option<u64> {
    part.attributes
        .iter()
        .find(|a| a.name == name)
        .and_then(|a| a.default.as_ref())
        .and_then(|d| d.trim().parse::<u64>().ok())
}

fn rust_u64_lit(n: u64) -> String {
    let raw = n.to_string();
    let mut out = String::new();
    for (i, ch) in raw.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('_');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn state_has_wcet(sm: &crate::ast::StateMachine) -> bool {
    sm.states.iter().any(|s| {
        s.entry_actions
            .iter()
            .chain(s.do_actions.iter())
            .chain(s.exit_actions.iter())
            .any(|a| a.wcet_us.is_some())
    })
}

fn emit_timing_constants(out: &mut String, part: &PartDef) {
    if let Some(p) = attr_u64(part, "execution_period_ms") {
        out.push_str(
            "    /// Periodic activation period from `SysML` `execution_period_ms`, in microseconds.\n",
        );
        let us = rust_u64_lit(p * 1000);
        buf::append(
            out,
            format_args!("    pub const EXECUTION_PERIOD_US: u64 = {us};\n"),
        );
    }
    if let Some(d) = attr_u64(part, "debounce_period_ms") {
        out.push_str(
            "    /// Event-driven polling period from `SysML` `debounce_period_ms`, in microseconds.\n",
        );
        let us = rust_u64_lit(d * 1000);
        buf::append(
            out,
            format_args!("    pub const DEBOUNCE_PERIOD_US: u64 = {us};\n"),
        );
    }
    if let Some(j) = attr_u64(part, "max_jitter_us") {
        out.push_str("    /// Allowed jitter from `SysML` `max_jitter_us`, in microseconds.\n");
        let lit = rust_u64_lit(j);
        buf::append(
            out,
            format_args!("    pub const MAX_JITTER_US: u64 = {lit};\n"),
        );
    }
    if let Some(m) = attr_u64(part, "max_execution_time_us") {
        out.push_str("    /// Deadline from `SysML` `max_execution_time_us`, in microseconds.\n");
        let lit = rust_u64_lit(m);
        buf::append(
            out,
            format_args!("    pub const MAX_EXECUTION_TIME_US: u64 = {lit};\n"),
        );
    }
    if let Some(p) = attr_u64(part, "priority") {
        out.push_str("    /// Scheduling priority from `SysML` `priority`.\n");
        buf::append(out, format_args!("    pub const PRIORITY: u8 = {p};\n"));
    }
    if let Some(c) = attr_u64(part, "core") {
        out.push_str("    /// Core affinity from `SysML` `core`.\n");
        buf::append(out, format_args!("    pub const CORE: u8 = {c};\n"));
    }
}

fn emit_wcet_constants(out: &mut String, part: &PartDef, sm: &crate::ast::StateMachine) {
    let mut max_wcet: u64 = 0;
    for state in &sm.states {
        let mut state_wcet: u64 = 0;
        let mut action_parts: Vec<String> = Vec::new();
        for action in state.entry_actions.iter().chain(state.do_actions.iter()) {
            if let Some(wcet) = action.wcet_us {
                state_wcet += wcet;
                action_parts.push(format!("{}({wcet})", action.name));
            }
        }
        max_wcet = max_wcet.max(state_wcet);

        let upper = to_screaming_snake(&state.name);
        let comment = if action_parts.is_empty() {
            String::new()
        } else {
            format!(" // {}", action_parts.join(" + "))
        };
        let lit = rust_u64_lit(state_wcet);
        let state_name = &state.name;
        buf::append(
            out,
            format_args!(
                "    /// WCET budget for `{state_name}` state (entry + do actions), in microseconds.\n",
            ),
        );
        buf::append(
            out,
            format_args!("    pub const WCET_{upper}_US: u64 = {lit};{comment}\n"),
        );
    }

    out.push_str("    /// Maximum WCET across all states, in microseconds.\n");
    let max_lit = rust_u64_lit(max_wcet);
    buf::append(
        out,
        format_args!("    pub const WCET_MAX_US: u64 = {max_lit};\n"),
    );

    let state_enum = format!("{}State", part.name);
    out.push_str("    /// Look up the WCET budget for a given state.\n");
    out.push_str("    #[must_use]\n");
    buf::append(
        out,
        format_args!("    pub const fn wcet_for_state(state: {state_enum}) -> u64 {{\n"),
    );
    out.push_str("        match state {\n");
    for state in &sm.states {
        let upper = to_screaming_snake(&state.name);
        let variant = to_pascal_case(&state.name);
        buf::append(
            out,
            format_args!("            {state_enum}::{variant} => Self::WCET_{upper}_US,\n"),
        );
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
}

/// Generate per-actor associated constants (WCET budgets and timing attributes).
fn generate_actor_constants(package: &Package) -> String {
    let mut out = String::new();
    for part in &package.parts {
        let Some(sm) = &part.state_machine else {
            continue;
        };

        let has_wcet = state_has_wcet(sm);
        let has_timing = attr_u64(part, "execution_period_ms").is_some()
            || attr_u64(part, "debounce_period_ms").is_some()
            || attr_u64(part, "priority").is_some()
            || attr_u64(part, "core").is_some()
            || attr_u64(part, "max_jitter_us").is_some()
            || attr_u64(part, "max_execution_time_us").is_some();

        if !has_wcet && !has_timing {
            continue;
        }

        buf::append(&mut out, format_args!("\nimpl {} {{\n", part.name));
        if has_timing {
            emit_timing_constants(&mut out, part);
        }
        if has_wcet {
            emit_wcet_constants(&mut out, part, sm);
        }
        out.push_str("}\n");
    }
    out
}
