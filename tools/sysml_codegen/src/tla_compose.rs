//! Composed TLA+ specification generation for multi-actor systems.
//!
//! Given a system part def with part instances and connections, generates a
//! single TLA+ module that composes all actor state machines with bounded
//! channel buffers for inter-actor communication.

use crate::ast::*;
use crate::tla_expr;
use std::collections::{BTreeSet, HashMap};

/// Channel metadata derived from a connection.
struct ChannelInfo {
    /// TLA+ variable name for the channel buffer.
    var_name: String,
    /// Source port definition (to extract signal fields).
    port_signals: Vec<Signal>,
    /// Channel capacity (from connection).
    capacity: usize,
    /// Connection reference.
    from_part: String,
    from_port: String,
    to_part: String,
    to_port: String,
}

/// Per-actor variable info for the composed spec.
struct ActorVarInfo {
    /// Instance name prefix (e.g., "button").
    prefix: String,
    /// State variable names (prefixed).
    state_var: String,
    /// State names from the state machine.
    state_names: Vec<String>,
    /// Mutable variables (prefixed name, type, default).
    vars: Vec<(String, String, String)>,
    /// Constants (prefixed name, default).
    constants: Vec<(String, Option<String>)>,
}

/// Render a composed TLA+ spec for a system part def.
/// Returns `(tla_content, cfg_content)`.
pub fn render_composed_tla(
    system: &PartDef,
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> (String, String) {
    // Build channel info from connections
    let channels = build_channels(system, parts, all_ports);

    // Build per-actor variable info
    let actors = build_actor_info(system, parts, all_ports);

    let tla = render_tla_module(system, &actors, &channels, parts, all_ports);
    let cfg = render_cfg(system, &actors, &channels);

    (tla, cfg)
}

fn build_channels(
    system: &PartDef,
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> Vec<ChannelInfo> {
    let mut channels = Vec::new();

    for conn in &system.connections {
        // Find the source port's signals
        let port_signals = find_port_signals(
            &conn.from_part, &conn.from_port, parts, all_ports,
        );

        let var_name = format!("chan_{}_to_{}", conn.from_part, conn.to_part);

        channels.push(ChannelInfo {
            var_name,
            port_signals,
            capacity: conn.capacity,
            from_part: conn.from_part.clone(),
            from_port: conn.from_port.clone(),
            to_part: conn.to_part.clone(),
            to_port: conn.to_port.clone(),
        });
    }

    channels
}

/// Find the signals for a port on a part instance.
fn find_port_signals(
    part_name: &str,
    port_name: &str,
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> Vec<Signal> {
    if let Some(part_def) = parts.get(part_name) {
        // Find the port on this part def
        if let Some(port) = part_def.ports.iter().find(|p| p.name == port_name) {
            return port.signals.clone();
        }
    }
    // Fallback: search all port defs by name
    for port_def in all_ports {
        if port_def.name == port_name {
            return port_def.signals.clone();
        }
    }
    Vec::new()
}

fn build_actor_info(
    system: &PartDef,
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> Vec<ActorVarInfo> {
    let mut actors = Vec::new();

    for inst in &system.part_instances {
        let Some(part_def) = parts.get(&inst.name) else {
            continue;
        };
        let Some(sm) = &part_def.state_machine else {
            continue;
        };

        let prefix = &inst.name;
        let state_var = format!("{prefix}_state");
        let state_names: Vec<String> = sm.states.iter().map(|s| s.name.clone()).collect();

        // Classify attributes
        let mut assigned: BTreeSet<String> = BTreeSet::new();
        for state in &sm.states {
            for action in state.entry_actions.iter()
                .chain(state.do_actions.iter())
                .chain(state.exit_actions.iter())
            {
                if let Some(body) = &action.body {
                    for (var, _) in tla_expr::parse_assignments(body) {
                        assigned.insert(var);
                    }
                }
            }
        }
        for t in &sm.transitions {
            for action in &t.actions {
                if let Some(body) = &action.body {
                    for (var, _) in tla_expr::parse_assignments(body) {
                        assigned.insert(var);
                    }
                }
            }
        }

        // Accept inputs
        let mut input_names: BTreeSet<String> = BTreeSet::new();
        for t in &sm.transitions {
            if t.is_accept {
                if let Some(cond) = &t.condition {
                    for word in cond.split(" and ") {
                        let word = word.trim();
                        let var_name = extract_accept_var_name(word);
                        input_names.insert(var_name);
                    }
                }
            }
        }

        let mut vars = Vec::new();
        let mut constants = Vec::new();

        for attr in &part_def.attributes {
            let prefixed = format!("{prefix}_{}", attr.name);
            if input_names.contains(&attr.name) || assigned.contains(&attr.name) {
                let default = tla_default(&attr.typ, &attr.default);
                vars.push((prefixed, attr.typ.clone(), default));
            } else {
                constants.push((prefixed, attr.default.clone()));
            }
        }

        // Add port signal variables (needed for Send/Receive actions).
        // If a signal name matches a constant attribute, promote it to a variable.
        // For ports with unresolved signals (cross-file port defs), look up from all_ports.
        let resolved_ports: Vec<&Port> = part_def.ports.iter()
            .map(|p| {
                if p.signals.is_empty() {
                    // Try to resolve from global port defs
                    all_ports.iter()
                        .find(|pd| pd.name == p.typ)
                        .copied()
                        .unwrap_or(p)
                } else {
                    p
                }
            })
            .collect();
        for port in &resolved_ports {
            for sig in &port.signals {
                let prefixed = format!("{prefix}_{}", sig.name);
                if vars.iter().any(|(n, _, _)| *n == prefixed) {
                    // Already a mutable variable — skip
                    continue;
                }
                if let Some(pos) = constants.iter().position(|(n, _)| *n == prefixed) {
                    // Promote constant to variable (port signals need to be mutable)
                    let (_, default_opt) = constants.remove(pos);
                    let default = tla_default(&sig.typ, &default_opt);
                    vars.push((prefixed, sig.typ.clone(), default));
                } else {
                    // New port signal variable (not an attribute)
                    let default = tla_default(&sig.typ, &None);
                    vars.push((prefixed, sig.typ.clone(), default));
                }
            }
        }

        actors.push(ActorVarInfo {
            prefix: prefix.clone(),
            state_var,
            state_names,
            vars,
            constants,
        });
    }

    actors
}

fn extract_accept_var_name(cond: &str) -> String {
    let operators = [">=", "<=", "!=", "==", ">", "<"];
    for op in &operators {
        if let Some(pos) = cond.find(op) {
            return cond[..pos].trim().to_string();
        }
    }
    cond.to_string()
}

fn tla_default(typ: &str, default: &Option<String>) -> String {
    match default {
        Some(val) => tla_expr::sysml_expr_to_tla(val),
        None => match typ {
            "Real" | "Integer" => "0".to_string(),
            "Boolean" => "FALSE".to_string(),
            _ => "0".to_string(),
        },
    }
}

fn render_tla_module(
    system: &PartDef,
    actors: &[ActorVarInfo],
    channels: &[ChannelInfo],
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> String {
    let mut out = String::new();
    let name = &system.name;

    // Header
    let dashes = 75_usize.saturating_sub(name.len() + 9) / 2;
    let dash_str: String = std::iter::repeat('-').take(dashes).collect();
    out.push_str(&format!("{dash_str} MODULE {name} {dash_str}\n"));
    out.push_str("EXTENDS Integers, Naturals, Sequences\n\n");

    // CONSTANTS
    let all_constants: Vec<(&str, &Option<String>)> = actors.iter()
        .flat_map(|a| a.constants.iter().map(|(n, d)| (n.as_str(), d)))
        .collect();
    if !all_constants.is_empty() {
        out.push_str("CONSTANTS\n");
        for (i, (name, _)) in all_constants.iter().enumerate() {
            if i < all_constants.len() - 1 {
                out.push_str(&format!("    {name},\n"));
            } else {
                out.push_str(&format!("    {name}\n"));
            }
        }
        out.push('\n');
    }

    // VARIABLES
    out.push_str("VARIABLES\n");
    let mut all_vars: Vec<String> = Vec::new();
    for actor in actors {
        all_vars.push(actor.state_var.clone());
        for (v, _, _) in &actor.vars {
            all_vars.push(v.clone());
        }
    }
    for ch in channels {
        all_vars.push(ch.var_name.clone());
    }
    for (i, v) in all_vars.iter().enumerate() {
        if i < all_vars.len() - 1 {
            out.push_str(&format!("    {v},\n"));
        } else {
            out.push_str(&format!("    {v}\n"));
        }
    }
    out.push('\n');

    let vars_tuple = format!("<<{}>>", all_vars.join(", "));
    out.push_str(&format!("vars == {vars_tuple}\n\n"));

    // Per-actor States sets
    for actor in actors {
        let states_str: Vec<String> = actor.state_names.iter()
            .map(|s| format!("\"{s}\""))
            .collect();
        out.push_str(&format!("{}States == {{{}}}\n",
            capitalize_first(&actor.prefix),
            states_str.join(", ")));
    }
    out.push('\n');

    // Message record types for each channel (named by source part instance)
    let mut generated_msg_types: Vec<String> = Vec::new();
    for ch in channels {
        if !ch.port_signals.is_empty() {
            let msg_type_name = format!("{}Msg", capitalize_first(&ch.from_part));
            if generated_msg_types.contains(&msg_type_name) {
                continue;
            }
            let fields: Vec<String> = ch.port_signals.iter()
                .map(|s| format!("{}: {}", s.name, tla_type_for_signal(&s.typ)))
                .collect();
            out.push_str(&format!("{msg_type_name} == [{}]\n", fields.join(", ")));
            generated_msg_types.push(msg_type_name);
        }
    }
    if !channels.is_empty() {
        out.push('\n');
    }

    // Per-actor Init
    for actor in actors {
        let init_name = format!("Init{}", capitalize_first(&actor.prefix));
        out.push_str(&format!("{init_name} ==\n"));
        let initial_state = &actor.state_names[0];
        out.push_str(&format!("    /\\ {} = \"{initial_state}\"\n", actor.state_var));
        for (v, _, default) in &actor.vars {
            out.push_str(&format!("    /\\ {v} = {default}\n"));
        }
        out.push('\n');
    }

    // Channel Init
    if !channels.is_empty() {
        out.push_str("InitChannels ==\n");
        for ch in channels {
            out.push_str(&format!("    /\\ {} = <<>>\n", ch.var_name));
        }
        out.push('\n');
    }

    // Combined Init
    out.push_str("Init ==\n");
    for actor in actors {
        out.push_str(&format!("    /\\ Init{}\n", capitalize_first(&actor.prefix)));
    }
    if !channels.is_empty() {
        out.push_str("    /\\ InitChannels\n");
    }
    out.push('\n');

    // Per-actor Step actions (prefixed variables)
    for inst in &system.part_instances {
        let Some(part_def) = parts.get(&inst.name) else { continue };
        let Some(sm) = &part_def.state_machine else { continue };

        let prefix = &inst.name;
        let other_vars = collect_other_vars(&all_vars, prefix, channels);

        render_actor_steps(&mut out, prefix, sm, part_def, &other_vars, all_ports);
    }

    // Send actions
    for ch in channels {
        render_send_action(&mut out, ch, &all_vars);
    }

    // Receive actions
    for ch in channels {
        render_receive_action(&mut out, ch, &all_vars, parts);
    }

    // Step disjunctions per actor
    let mut all_step_names: Vec<String> = Vec::new();
    for inst in &system.part_instances {
        let Some(part_def) = parts.get(&inst.name) else { continue };
        let Some(sm) = &part_def.state_machine else { continue };

        let prefix = &inst.name;
        let step_name = format!("Step{}", capitalize_first(prefix));
        let state_steps: Vec<String> = sm.states.iter()
            .filter(|s| sm.transitions.iter().any(|t| t.from_state == s.name))
            .map(|s| format!("Step{}_{}", capitalize_first(prefix), s.name))
            .collect();

        if !state_steps.is_empty() {
            out.push_str(&format!("{step_name} == {}\n\n", state_steps.join(" \\/ ")));
            all_step_names.push(step_name);
        }
    }

    // Send/Receive action names (from_to_dest naming)
    let mut send_names: Vec<String> = Vec::new();
    let mut recv_names: Vec<String> = Vec::new();
    for ch in channels {
        send_names.push(format!("Send_{}_to_{}", ch.from_part, ch.to_part));
        recv_names.push(format!("Receive_{}_to_{}", ch.from_part, ch.to_part));
    }

    // Next
    let mut next_parts: Vec<String> = Vec::new();
    next_parts.extend(all_step_names.iter().cloned());
    next_parts.extend(send_names.iter().cloned());
    next_parts.extend(recv_names.iter().cloned());
    out.push_str(&format!("Next == {}\n\n", next_parts.join("\n    \\/ ")));

    // Spec with fairness
    out.push_str(&format!("Spec == Init /\\ [][Next]_{vars_tuple}\n"));
    for step_name in &all_step_names {
        out.push_str(&format!("    /\\ WF_{vars_tuple}({step_name})\n"));
    }
    for sn in &send_names {
        out.push_str(&format!("    /\\ SF_{vars_tuple}({sn})\n"));
    }
    for rn in &recv_names {
        out.push_str(&format!("    /\\ SF_{vars_tuple}({rn})\n"));
    }
    out.push('\n');

    // TypeInvariant
    out.push_str("TypeInvariant ==\n");
    for actor in actors {
        out.push_str(&format!("    /\\ {} \\in {}States\n",
            actor.state_var, capitalize_first(&actor.prefix)));
        for (v, typ, _) in &actor.vars {
            out.push_str(&format!("    /\\ {}\n", tla_type_constraint(v, typ)));
        }
    }
    out.push('\n');

    // ChannelBounded
    out.push_str("ChannelBounded ==\n");
    for ch in channels {
        out.push_str(&format!("    /\\ Len({}) <= {}\n", ch.var_name, ch.capacity));
    }
    out.push('\n');

    // Safety
    out.push_str("Safety == TypeInvariant /\\ ChannelBounded\n\n");

    // StateConstraint for bounded model checking
    let int_vars: Vec<&str> = actors.iter()
        .flat_map(|a| a.vars.iter()
            .filter(|(_, t, _)| t == "Integer" || t == "Real")
            .map(|(n, _, _)| n.as_str()))
        .collect();
    if !int_vars.is_empty() {
        out.push_str("StateConstraint ==\n");
        for v in &int_vars {
            out.push_str(&format!("    /\\ {v} \\in -10..10\n"));
        }
        out.push('\n');
    }

    // Liveness
    out.push_str("Liveness ==\n");
    for actor in actors {
        let initial = &actor.state_names[0];
        out.push_str(&format!("    /\\ []<>({} = \"{initial}\")\n", actor.state_var));
    }
    out.push('\n');

    // Footer
    let footer: String = std::iter::repeat('=').take(75).collect();
    out.push_str(&footer);
    out.push('\n');

    out
}

/// Collect all variable names that do NOT belong to this actor prefix,
/// plus channel variables. Used for UNCHANGED clauses.
fn collect_other_vars(all_vars: &[String], prefix: &str, channels: &[ChannelInfo]) -> Vec<String> {
    let actor_prefix = format!("{prefix}_");
    all_vars.iter()
        .filter(|v| {
            // Keep vars that don't start with this actor's prefix
            // OR are channel vars (channels are always unchanged in Step actions)
            !v.starts_with(&actor_prefix) || channels.iter().any(|c| c.var_name == **v)
        })
        .cloned()
        .collect()
}

/// Render per-state Step actions for one actor, with prefixed variable names.
fn render_actor_steps(
    out: &mut String,
    prefix: &str,
    sm: &StateMachine,
    part_def: &PartDef,
    other_vars: &[String],
    all_ports: &[&Port],
) {
    let state_var = format!("{prefix}_state");
    let cap_prefix = capitalize_first(prefix);

    // Build the set of mutable variable names (matching build_actor_info logic)
    let mut assigned: BTreeSet<String> = BTreeSet::new();
    for state in &sm.states {
        for action in state.entry_actions.iter()
            .chain(state.do_actions.iter())
            .chain(state.exit_actions.iter())
        {
            if let Some(body) = &action.body {
                for (var, _) in tla_expr::parse_assignments(body) {
                    assigned.insert(var);
                }
            }
        }
    }
    for t in &sm.transitions {
        for action in &t.actions {
            if let Some(body) = &action.body {
                for (var, _) in tla_expr::parse_assignments(body) {
                    assigned.insert(var);
                }
            }
        }
    }
    let mut input_names: BTreeSet<String> = BTreeSet::new();
    for t in &sm.transitions {
        if t.is_accept {
            if let Some(cond) = &t.condition {
                for word in cond.split(" and ") {
                    input_names.insert(extract_accept_var_name(word.trim()));
                }
            }
        }
    }

    // Actor var names = mutable attributes + port signals (excludes constants)
    let mut actor_var_names: Vec<String> = Vec::new();
    for attr in &part_def.attributes {
        if input_names.contains(&attr.name) || assigned.contains(&attr.name) {
            actor_var_names.push(format!("{prefix}_{}", attr.name));
        }
    }
    let resolved_ports: Vec<&Port> = part_def.ports.iter()
        .map(|p| {
            if p.signals.is_empty() {
                all_ports.iter()
                    .find(|pd| pd.name == p.typ)
                    .copied()
                    .unwrap_or(p)
            } else {
                p
            }
        })
        .collect();
    for port in &resolved_ports {
        for sig in &port.signals {
            let prefixed = format!("{prefix}_{}", sig.name);
            if !actor_var_names.contains(&prefixed) {
                actor_var_names.push(prefixed);
            }
        }
    }

    for state in &sm.states {
        let transitions: Vec<&Transition> = sm.transitions.iter()
            .filter(|t| t.from_state == state.name)
            .collect();

        if transitions.is_empty() {
            continue;
        }

        let step_name = format!("Step{cap_prefix}_{}", state.name);
        out.push_str(&format!("(* {prefix}/{}: transitions *)\n", state.name));
        out.push_str(&format!("{step_name} ==\n"));
        out.push_str(&format!("    /\\ {state_var} = \"{}\"\n", state.name));

        // Collect do-action assignments
        let do_assigns = collect_prefixed_assignments_from_state(state, "do", prefix);
        // Collect exit-action assignments
        let exit_assigns = collect_prefixed_assignments_from_state(state, "exit", prefix);

        for (var, expr) in &do_assigns {
            let tla_expr = prefix_expr(&tla_expr::sysml_expr_to_tla(expr), prefix, part_def);
            out.push_str(&format!("    /\\ {var}' = {tla_expr}\n"));
        }
        for (var, expr) in &exit_assigns {
            let tla_expr = prefix_expr(&tla_expr::sysml_expr_to_tla(expr), prefix, part_def);
            out.push_str(&format!("    /\\ {var}' = {tla_expr}\n"));
        }

        let do_exit_modified: BTreeSet<String> = do_assigns.iter()
            .chain(exit_assigns.iter())
            .map(|(v, _)| v.clone())
            .collect();

        // Separate conditional and unconditional
        let conditional: Vec<&&Transition> = transitions.iter()
            .filter(|t| t.condition.is_some())
            .collect();
        let unconditional: Vec<&&Transition> = transitions.iter()
            .filter(|t| t.condition.is_none())
            .collect();

        // Collect all vars modified in any branch
        let mut all_branch_modified: BTreeSet<String> = do_exit_modified.clone();
        for t in &transitions {
            let entry = collect_prefixed_entry_assignments(sm, &t.to_state, prefix);
            let trans = collect_prefixed_transition_assignments(t, prefix);
            for (v, _) in entry.iter().chain(trans.iter()) {
                all_branch_modified.insert(v.clone());
            }
        }

        if conditional.is_empty() && unconditional.len() == 1 {
            // Simple unconditional
            let t = unconditional[0];
            let trans_assigns = collect_prefixed_transition_assignments(t, prefix);
            let entry_assigns = collect_prefixed_entry_assignments(sm, &t.to_state, prefix);
            out.push_str(&format!("    /\\ {state_var}' = \"{}\"\n", t.to_state));
            for (var, expr) in &trans_assigns {
                if !do_exit_modified.contains(var) {
                    let tla_expr = prefix_expr(&tla_expr::sysml_expr_to_tla(expr), prefix, part_def);
                    out.push_str(&format!("    /\\ {var}' = {tla_expr}\n"));
                }
            }
            for (var, expr) in &entry_assigns {
                if !do_exit_modified.contains(var) {
                    let tla_expr = prefix_expr(&tla_expr::sysml_expr_to_tla(expr), prefix, part_def);
                    out.push_str(&format!("    /\\ {var}' = {tla_expr}\n"));
                }
            }
            // Vars modified in branch but not set → unchanged
            let branch_set: BTreeSet<String> = trans_assigns.iter()
                .chain(entry_assigns.iter())
                .map(|(v, _)| v.clone())
                .collect();
            for var in &all_branch_modified {
                if !do_exit_modified.contains(var) && !branch_set.contains(var) {
                    out.push_str(&format!("    /\\ {var}' = {var}\n"));
                }
            }
        } else if !conditional.is_empty() {
            // IF/ELSE chain
            out.push_str("    /\\ ");
            for (i, t) in conditional.iter().enumerate() {
                let cond = t.condition.as_ref().expect("conditional has condition");
                let tla_cond = prefix_expr(&tla_expr::sysml_condition_to_tla(cond), prefix, part_def);

                if i == 0 {
                    out.push_str(&format!("IF {tla_cond}\n"));
                } else {
                    out.push_str(&format!("       ELSE IF {tla_cond}\n"));
                }

                let trans_assigns = collect_prefixed_transition_assignments(t, prefix);
                let entry_assigns = collect_prefixed_entry_assignments(sm, &t.to_state, prefix);
                out.push_str(&format!("       THEN /\\ {state_var}' = \"{}\"\n", t.to_state));
                for (var, expr) in &trans_assigns {
                    if !do_exit_modified.contains(var) {
                        let tla_expr = prefix_expr(&tla_expr::sysml_expr_to_tla(expr), prefix, part_def);
                        out.push_str(&format!("            /\\ {var}' = {tla_expr}\n"));
                    }
                }
                for (var, expr) in &entry_assigns {
                    if !do_exit_modified.contains(var) {
                        let tla_expr = prefix_expr(&tla_expr::sysml_expr_to_tla(expr), prefix, part_def);
                        out.push_str(&format!("            /\\ {var}' = {tla_expr}\n"));
                    }
                }
                // Branch unchanged
                let branch_set: BTreeSet<String> = trans_assigns.iter()
                    .chain(entry_assigns.iter())
                    .map(|(v, _)| v.clone())
                    .collect();
                for var in &all_branch_modified {
                    if !do_exit_modified.contains(var) && !branch_set.contains(var) {
                        out.push_str(&format!("            /\\ {var}' = {var}\n"));
                    }
                }
            }
            // ELSE
            if !unconditional.is_empty() {
                let t = unconditional[0];
                let trans_assigns = collect_prefixed_transition_assignments(t, prefix);
                let entry_assigns = collect_prefixed_entry_assignments(sm, &t.to_state, prefix);
                out.push_str(&format!("       ELSE /\\ {state_var}' = \"{}\"\n", t.to_state));
                for (var, expr) in &trans_assigns {
                    if !do_exit_modified.contains(var) {
                        let tla_expr = prefix_expr(&tla_expr::sysml_expr_to_tla(expr), prefix, part_def);
                        out.push_str(&format!("            /\\ {var}' = {tla_expr}\n"));
                    }
                }
                for (var, expr) in &entry_assigns {
                    if !do_exit_modified.contains(var) {
                        let tla_expr = prefix_expr(&tla_expr::sysml_expr_to_tla(expr), prefix, part_def);
                        out.push_str(&format!("            /\\ {var}' = {tla_expr}\n"));
                    }
                }
                let branch_set: BTreeSet<String> = trans_assigns.iter()
                    .chain(entry_assigns.iter())
                    .map(|(v, _)| v.clone())
                    .collect();
                for var in &all_branch_modified {
                    if !do_exit_modified.contains(var) && !branch_set.contains(var) {
                        out.push_str(&format!("            /\\ {var}' = {var}\n"));
                    }
                }
            } else {
                out.push_str(&format!("       ELSE /\\ {state_var}' = {state_var}\n"));
                for var in &all_branch_modified {
                    if !do_exit_modified.contains(var) {
                        out.push_str(&format!("            /\\ {var}' = {var}\n"));
                    }
                }
            }
        }

        // Unchanged: actor vars not modified + other actors + channels
        let unchanged_actor: Vec<String> = actor_var_names.iter()
            .filter(|v| !all_branch_modified.contains(*v))
            .cloned()
            .collect();
        let mut unchanged: Vec<String> = unchanged_actor;
        unchanged.extend(other_vars.iter().cloned());
        if !unchanged.is_empty() {
            out.push_str(&format!("    /\\ UNCHANGED <<{}>>\n", unchanged.join(", ")));
        }
        out.push('\n');
    }
}

/// Collect assignments from a state's do or exit actions, with prefixed variable names.
fn collect_prefixed_assignments_from_state(
    state: &State,
    action_kind: &str,
    prefix: &str,
) -> Vec<(String, String)> {
    let actions = match action_kind {
        "do" => &state.do_actions,
        "exit" => &state.exit_actions,
        "entry" => &state.entry_actions,
        _ => return Vec::new(),
    };

    let mut result = Vec::new();
    for action in actions {
        if let Some(body) = &action.body {
            for (var, expr) in tla_expr::parse_assignments(body) {
                result.push((format!("{prefix}_{var}"), expr));
            }
        }
    }
    result
}

fn collect_prefixed_entry_assignments(
    sm: &StateMachine,
    target_name: &str,
    prefix: &str,
) -> Vec<(String, String)> {
    if let Some(target) = sm.states.iter().find(|s| s.name == target_name) {
        collect_prefixed_assignments_from_state(target, "entry", prefix)
    } else {
        Vec::new()
    }
}

fn collect_prefixed_transition_assignments(
    t: &Transition,
    prefix: &str,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for action in &t.actions {
        if let Some(body) = &action.body {
            for (var, expr) in tla_expr::parse_assignments(body) {
                result.push((format!("{prefix}_{var}"), expr));
            }
        }
    }
    result
}

/// Prefix bare variable references in a TLA+ expression with the actor prefix.
fn prefix_expr(expr: &str, prefix: &str, part_def: &PartDef) -> String {
    let mut result = expr.to_string();
    // Sort attributes by name length descending to avoid partial replacements
    let mut attrs: Vec<&Attribute> = part_def.attributes.iter().collect();
    attrs.sort_by(|a, b| b.name.len().cmp(&a.name.len()));

    for attr in attrs {
        let pattern = &attr.name;
        let replacement = format!("{prefix}_{}", attr.name);
        result = word_replace(&result, pattern, &replacement);
    }
    result
}

/// Replace whole words only (not inside other identifiers).
fn word_replace(text: &str, pattern: &str, replacement: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;

    loop {
        match remaining.find(pattern) {
            None => break,
            Some(pos) => {
                let before_ok = if pos == 0 {
                    true
                } else {
                    let b = remaining.as_bytes()[pos - 1];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                let after_pos = pos + pattern.len();
                let after_ok = if after_pos >= remaining.len() {
                    true
                } else {
                    let b = remaining.as_bytes()[after_pos];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };

                if before_ok && after_ok {
                    result.push_str(&remaining[..pos]);
                    result.push_str(replacement);
                    remaining = &remaining[after_pos..];
                } else {
                    result.push_str(&remaining[..after_pos]);
                    remaining = &remaining[after_pos..];
                }
            }
        }
    }
    result.push_str(remaining);
    result
}

/// Render a Send action for a channel.
fn render_send_action(out: &mut String, ch: &ChannelInfo, all_vars: &[String]) {
    let action_name = format!("Send_{}_to_{}", ch.from_part, ch.to_part);
    let prefix = &ch.from_part;

    out.push_str(&format!("(* Send from {}.{} into {} *)\n", ch.from_part, ch.from_port, ch.var_name));
    out.push_str(&format!("{action_name} ==\n"));
    out.push_str(&format!("    /\\ Len({}) < {}\n", ch.var_name, ch.capacity));

    // Build message record from port signals (prefixed with source actor)
    if !ch.port_signals.is_empty() {
        let fields: Vec<String> = ch.port_signals.iter()
            .map(|s| format!("{} |-> {prefix}_{}", s.name, s.name))
            .collect();
        out.push_str(&format!("    /\\ {}' = Append({}, [{}])\n",
            ch.var_name, ch.var_name, fields.join(", ")));
    } else {
        out.push_str(&format!("    /\\ {}' = Append({}, <<>>)\n", ch.var_name, ch.var_name));
    }

    // UNCHANGED: all vars except this channel
    let unchanged: Vec<&str> = all_vars.iter()
        .filter(|v| **v != ch.var_name)
        .map(|v| v.as_str())
        .collect();
    if !unchanged.is_empty() {
        out.push_str(&format!("    /\\ UNCHANGED <<{}>>\n", unchanged.join(", ")));
    }
    out.push('\n');
}

/// Render a Receive action for a channel.
fn render_receive_action(
    out: &mut String,
    ch: &ChannelInfo,
    all_vars: &[String],
    _parts: &HashMap<String, &PartDef>,
) {
    let action_name = format!("Receive_{}_to_{}", ch.from_part, ch.to_part);
    let to_prefix = &ch.to_part;

    out.push_str(&format!("(* Receive into {}.{} from {} *)\n", ch.to_part, ch.to_port, ch.var_name));
    out.push_str(&format!("{action_name} ==\n"));
    out.push_str(&format!("    /\\ Len({}) > 0\n", ch.var_name));
    out.push_str(&format!("    /\\ LET msg == Head({}) IN\n", ch.var_name));

    // Map signal fields to destination actor's variables
    // The destination port is conjugated — signals that were "out" on the source
    // become readable inputs on the destination
    let mut modified_vars: Vec<String> = Vec::new();
    if !ch.port_signals.is_empty() {
        for sig in &ch.port_signals {
            let dest_var = format!("{to_prefix}_{}", sig.name);
            out.push_str(&format!("       /\\ {dest_var}' = msg.{}\n", sig.name));
            modified_vars.push(dest_var);
        }
    }
    out.push_str(&format!("       /\\ {}' = Tail({})\n", ch.var_name, ch.var_name));

    // UNCHANGED: all vars except modified ones and the channel
    let mut changed_set: BTreeSet<String> = modified_vars.iter().cloned().collect();
    changed_set.insert(ch.var_name.clone());
    let unchanged: Vec<&str> = all_vars.iter()
        .filter(|v| !changed_set.contains(*v))
        .map(|v| v.as_str())
        .collect();
    if !unchanged.is_empty() {
        out.push_str(&format!("    /\\ UNCHANGED <<{}>>\n", unchanged.join(", ")));
    }
    out.push('\n');
}

fn render_cfg(
    _system: &PartDef,
    actors: &[ActorVarInfo],
    _channels: &[ChannelInfo],
) -> String {
    let mut out = String::new();
    out.push_str("SPECIFICATION Spec\n");

    // Constants
    let all_constants: Vec<(&str, &Option<String>)> = actors.iter()
        .flat_map(|a| a.constants.iter().map(|(n, d)| (n.as_str(), d)))
        .collect();
    if !all_constants.is_empty() {
        out.push_str("CONSTANTS\n");
        for (name, default) in &all_constants {
            let val = match default {
                Some(d) => tla_expr::sysml_expr_to_tla(d),
                None => "0".to_string(),
            };
            out.push_str(&format!("    {name} = {val}\n"));
        }
    }

    // State constraint
    let has_int_vars = actors.iter()
        .any(|a| a.vars.iter().any(|(_, t, _)| t == "Integer" || t == "Real"));
    if has_int_vars {
        out.push_str("CONSTRAINT StateConstraint\n");
    }

    out.push_str("INVARIANTS\n");
    out.push_str("    Safety\n");

    out.push_str("PROPERTIES\n");
    out.push_str("    Liveness\n");

    out
}

fn tla_type_for_signal(typ: &str) -> &'static str {
    match typ {
        "Boolean" => "BOOLEAN",
        "Integer" | "Real" => "Int",
        _ => "Int",
    }
}

fn tla_type_constraint(name: &str, typ: &str) -> String {
    match typ {
        "Boolean" => format!("{name} \\in BOOLEAN"),
        "Integer" | "Real" => format!("{name} \\in Int"),
        _ => format!("{name} \\in Int"),
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
    }
}
