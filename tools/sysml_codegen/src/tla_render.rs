//! TLA+ specification generation from SysML AST.
//!
//! Generates `.tla` and `.cfg` files directly from the SysML AST (Package).
//! Each PartDef with a state machine produces one TLA+ module.

use crate::ast::*;
use crate::tla_expr;
use std::collections::BTreeSet;

/// Render TLA+ specs for all parts with state machines.
/// Returns `(part_name, tla_content, cfg_content)` per part.
pub fn render_tla(package: &Package) -> Vec<(String, String, String)> {
    let mut results = Vec::new();
    for part in &package.parts {
        if let Some(sm) = &part.state_machine {
            let info = classify_attributes(part, sm);
            let tla = render_tla_module(part, sm, &info);
            let cfg = render_cfg_file(part, &info);
            results.push((part.name.clone(), tla, cfg));
        }
    }
    results
}

/// Classification of part attributes for TLA+ generation.
struct AttrInfo {
    /// Constants: never assigned in any action body, not accept inputs.
    constants: Vec<ConstantInfo>,
    /// Input variables: appear in `accept` transitions.
    inputs: BTreeSet<String>,
    /// State variables: mutable attributes (assigned somewhere).
    state_vars: Vec<VarInfo>,
}

struct ConstantInfo {
    name: String,
    default: Option<String>,
}

struct VarInfo {
    name: String,
    typ: String,
    default: String,
}

/// Classify attributes into constants, inputs, and state variables.
fn classify_attributes(part: &PartDef, sm: &StateMachine) -> AttrInfo {
    // Collect all variables assigned in any action body
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

    // Collect accept (input) variables
    let mut inputs: BTreeSet<String> = BTreeSet::new();
    for t in &sm.transitions {
        if t.is_accept {
            if let Some(cond) = &t.condition {
                // The condition from accept is the event name (variable name)
                // It may be combined with "and" for accept EVENT if COND
                // Extract bare variable names that are the accept event
                for word in cond.split(" and ") {
                    let word = word.trim();
                    // Skip comparison expressions (contain operators)
                    if !word.contains(">=") && !word.contains("<=")
                        && !word.contains('>') && !word.contains('<')
                        && !word.contains("==") && !word.contains("!=")
                    {
                        inputs.insert(word.to_string());
                    }
                }
            }
        }
    }

    let mut constants = Vec::new();
    let mut state_vars = Vec::new();

    for attr in &part.attributes {
        let name = &attr.name;
        if inputs.contains(name) {
            // Input variable — handled separately
            state_vars.push(VarInfo {
                name: name.clone(),
                typ: attr.typ.clone(),
                default: tla_default(&attr.typ, &attr.default),
            });
        } else if assigned.contains(name) {
            // Mutable state variable
            state_vars.push(VarInfo {
                name: name.clone(),
                typ: attr.typ.clone(),
                default: tla_default(&attr.typ, &attr.default),
            });
        } else {
            // Constant
            constants.push(ConstantInfo {
                name: name.clone(),
                default: attr.default.clone(),
            });
        }
    }

    AttrInfo {
        constants,
        inputs,
        state_vars,
    }
}

/// Get TLA+ default value for a SysML type.
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

/// Get TLA+ type constraint for an attribute.
fn tla_type_constraint(name: &str, typ: &str) -> String {
    match typ {
        "Boolean" => format!("{name} \\in BOOLEAN"),
        "Integer" | "Real" => format!("{name} \\in Int"),
        _ => format!("{name} \\in Int"),
    }
}

/// Render the complete TLA+ module for a part.
fn render_tla_module(part: &PartDef, sm: &StateMachine, info: &AttrInfo) -> String {
    let mut out = String::new();
    let name = &part.name;

    // Header
    let dashes = 75_usize.saturating_sub(name.len() + 9) / 2;
    let dash_str: String = std::iter::repeat('-').take(dashes).collect();
    out.push_str(&format!("{dash_str} MODULE {name} {dash_str}\n"));
    out.push_str("EXTENDS Integers, Naturals\n\n");

    // CONSTANTS
    if !info.constants.is_empty() {
        out.push_str("CONSTANTS\n");
        for (i, c) in info.constants.iter().enumerate() {
            if i < info.constants.len() - 1 {
                out.push_str(&format!("    {},\n", c.name));
            } else {
                out.push_str(&format!("    {}\n", c.name));
            }
        }
        out.push('\n');
    }

    // VARIABLES
    out.push_str("VARIABLES\n");
    let mut all_vars: Vec<&str> = vec!["state"];
    for v in &info.state_vars {
        all_vars.push(&v.name);
    }
    for (i, v) in all_vars.iter().enumerate() {
        if i < all_vars.len() - 1 {
            out.push_str(&format!("    {v},\n"));
        } else {
            out.push_str(&format!("    {v}\n"));
        }
    }
    out.push('\n');

    // vars tuple
    let vars_tuple = format!("<<{}>>", all_vars.join(", "));
    out.push_str(&format!("vars == {vars_tuple}\n\n"));

    // States set
    let state_names: Vec<String> = sm.states.iter()
        .map(|s| format!("\"{}\"", s.name))
        .collect();
    out.push_str(&format!("States == {{{}}}\n\n", state_names.join(", ")));

    // TypeInvariant
    out.push_str("TypeInvariant ==\n");
    out.push_str("    /\\ state \\in States\n");
    for v in &info.state_vars {
        out.push_str(&format!("    /\\ {}\n", tla_type_constraint(&v.name, &v.typ)));
    }
    out.push('\n');

    // Init
    out.push_str("Init ==\n");
    let initial_state = &sm.states[0].name;
    out.push_str(&format!("    /\\ state = \"{initial_state}\"\n"));
    for v in &info.state_vars {
        out.push_str(&format!("    /\\ {} = {}\n", v.name, v.default));
    }
    out.push('\n');

    // Step actions (one per state with outgoing transitions)
    let step_actions = render_step_actions(sm, info);
    out.push_str(&step_actions);

    // Step disjunction
    let states_with_transitions = get_states_with_transitions(sm);
    if !states_with_transitions.is_empty() {
        out.push_str("Step == ");
        let step_names: Vec<String> = states_with_transitions.iter()
            .map(|s| format!("Step{s}"))
            .collect();
        out.push_str(&step_names.join(" \\/ "));
        out.push_str("\n\n");
    }

    // Env actions: one sub-action per input variable.
    // Each Env_v sets one input nondeterministically, others unchanged.
    // This models independent input arrival and prevents priority starvation
    // in IF-THEN-ELSE guard chains.
    let has_env = !info.inputs.is_empty();
    if has_env {
        let input_names: Vec<&String> = info.inputs.iter().collect();
        let non_input_vars: Vec<&str> = std::iter::once("state")
            .chain(info.state_vars.iter()
                .filter(|v| !info.inputs.contains(&v.name))
                .map(|v| v.name.as_str()))
            .collect();

        for input_name in &input_names {
            let typ = part.attributes.iter()
                .find(|a| &a.name == *input_name)
                .map(|a| a.typ.as_str())
                .unwrap_or("Boolean");
            let domain = match typ {
                "Boolean" => "BOOLEAN",
                "Integer" | "Real" => "Int",
                _ => "BOOLEAN",
            };
            out.push_str(&format!("Env_{input_name} ==\n"));
            out.push_str(&format!("    /\\ {input_name}' \\in {domain}\n"));
            let other_inputs: Vec<&str> = input_names.iter()
                .filter(|n| *n != input_name)
                .map(|n| n.as_str())
                .collect();
            let mut unchanged: Vec<&str> = non_input_vars.clone();
            unchanged.extend(other_inputs);
            if !unchanged.is_empty() {
                out.push_str(&format!("    /\\ UNCHANGED <<{}>>\n", unchanged.join(", ")));
            }
            out.push('\n');
        }

        let env_names: Vec<String> = input_names.iter()
            .map(|n| format!("Env_{n}"))
            .collect();
        out.push_str(&format!("Env == {}\n\n", env_names.join(" \\/ ")));
    }

    // Next and Spec
    if has_env {
        out.push_str("Next == Step \\/ Env\n\n");
    } else {
        out.push_str("Next == Step\n\n");
    }

    // EnvFairness: SF_vars(Env_v /\ v' = TRUE) per boolean input
    if has_env {
        let input_names: Vec<&String> = info.inputs.iter().collect();
        let fairness_expr = |input_name: &str| -> String {
            let typ = part.attributes.iter()
                .find(|a| a.name == input_name)
                .map(|a| a.typ.as_str())
                .unwrap_or("Boolean");
            match typ {
                "Boolean" => format!("SF_{vars_tuple}(Env_{input_name} /\\ {input_name}' = TRUE)"),
                _ => format!("SF_{vars_tuple}(Env_{input_name} /\\ {input_name}')"),
            }
        };

        if input_names.len() == 1 {
            out.push_str(&format!(
                "EnvFairness == {}\n\n",
                fairness_expr(input_names[0])
            ));
        } else {
            out.push_str("EnvFairness ==\n");
            for input_name in &input_names {
                out.push_str(&format!(
                    "    /\\ {}\n",
                    fairness_expr(input_name)
                ));
            }
            out.push('\n');
        }
    }

    // Spec: SF for open systems (env can break WF's "continuously enabled" criterion),
    // WF for closed systems
    if has_env {
        out.push_str(&format!("Spec == Init /\\ [][Next]_{vars_tuple} /\\ SF_{vars_tuple}(Step) /\\ EnvFairness\n\n"));
    } else {
        out.push_str(&format!("Spec == Init /\\ [][Next]_{vars_tuple} /\\ WF_{vars_tuple}(Step)\n\n"));
    }

    // StateConstraint for bounded model checking
    let int_vars: Vec<&VarInfo> = info.state_vars.iter()
        .filter(|v| v.typ == "Integer" || v.typ == "Real")
        .collect();
    if !int_vars.is_empty() {
        out.push_str("StateConstraint ==\n");
        for (i, v) in int_vars.iter().enumerate() {
            let prefix = if i == 0 { "    /\\ " } else { "    /\\ " };
            out.push_str(&format!("{prefix}{} \\in -100..100\n", v.name));
        }
        out.push('\n');
    }

    // Safety
    out.push_str("Safety == state \\in States\n\n");

    // Liveness: for closed systems and open systems where all states cycle
    let cycles = all_states_have_transitions(sm);
    if !has_env || (has_env && cycles) {
        out.push_str(&format!("Liveness == []<>(state = \"{initial_state}\")\n\n"));
    }

    // Footer
    let footer_len = 75;
    let footer: String = std::iter::repeat('=').take(footer_len).collect();
    out.push_str(&footer);
    out.push('\n');

    out
}

/// Get list of state names that have outgoing transitions.
fn get_states_with_transitions(sm: &StateMachine) -> Vec<String> {
    let mut result = Vec::new();
    for state in &sm.states {
        let has_outgoing = sm.transitions.iter().any(|t| t.from_state == state.name);
        if has_outgoing {
            result.push(state.name.clone());
        }
    }
    result
}

/// Check if all states have outgoing transitions (no terminal states).
/// If true, the actor cycles and liveness can be generated.
fn all_states_have_transitions(sm: &StateMachine) -> bool {
    sm.states.iter().all(|s| {
        sm.transitions.iter().any(|t| t.from_state == s.name)
    })
}


/// Render all StepX actions.
fn render_step_actions(sm: &StateMachine, info: &AttrInfo) -> String {
    let mut out = String::new();

    // All variable names (excluding "state")
    let all_var_names: Vec<&str> = info.state_vars.iter()
        .map(|v| v.name.as_str())
        .collect();

    for state in &sm.states {
        // Collect transitions from this state
        let transitions: Vec<&Transition> = sm.transitions.iter()
            .filter(|t| t.from_state == state.name)
            .collect();

        if transitions.is_empty() {
            continue;
        }

        // Collect do-actions for this state
        let do_assignments = collect_do_assignments(state);
        // Collect variables modified by do-actions
        let do_modified: BTreeSet<String> = do_assignments.iter()
            .map(|(var, _)| var.clone())
            .collect();

        // Separate conditional and unconditional transitions
        let conditional: Vec<&&Transition> = transitions.iter()
            .filter(|t| t.condition.is_some())
            .collect();
        let unconditional: Vec<&&Transition> = transitions.iter()
            .filter(|t| t.condition.is_none())
            .collect();

        // Comment
        out.push_str(&format!("(* {}: ", state.name));
        if !do_assignments.is_empty() {
            let action_names: Vec<&str> = state.do_actions.iter()
                .map(|a| a.name.as_str())
                .collect();
            out.push_str(&format!("do {}", action_names.join(", ")));
            if !transitions.is_empty() {
                out.push_str(", ");
            }
        }
        if !conditional.is_empty() && unconditional.is_empty() {
            out.push_str("guard-based transition");
        } else if conditional.is_empty() && !unconditional.is_empty() {
            out.push_str(&format!("unconditional to {}", unconditional[0].to_state));
        } else if !conditional.is_empty() && !unconditional.is_empty() {
            out.push_str("conditional transitions with fallback");
        } else {
            out.push_str("transition");
        }
        out.push_str(" *)\n");

        out.push_str(&format!("Step{} ==\n", state.name));
        out.push_str(&format!("    /\\ state = \"{}\"\n", state.name));

        // Collect all variables modified by any branch (entry actions of target states)
        let mut all_branch_modified: BTreeSet<String> = BTreeSet::new();
        all_branch_modified.extend(do_modified.iter().cloned());
        for t in &transitions {
            let entry_assigns = collect_entry_assignments_for_target(sm, &t.to_state);
            for (var, _) in &entry_assigns {
                all_branch_modified.insert(var.clone());
            }
        }

        // Variables not modified in ANY branch or do-action → outer UNCHANGED
        let unchanged_vars: Vec<&str> = all_var_names.iter()
            .filter(|v| !all_branch_modified.contains(**v))
            .copied()
            .collect();

        // If do-actions modify variables that appear in guards, use LET bindings
        let needs_let = !do_assignments.is_empty() && conditional.iter().any(|t| {
            if let Some(cond) = &t.condition {
                do_modified.iter().any(|var| cond.contains(var.as_str()))
            } else {
                false
            }
        });

        if needs_let {
            // LET binding approach
            out.push_str("    /\\ LET ");
            let mut first_let = true;
            for (var, expr) in &do_assignments {
                let tla_expr = tla_expr::sysml_expr_to_tla(expr);
                let post_name = format!("{var}_post");
                if first_let {
                    out.push_str(&format!("{post_name} == {tla_expr}\n"));
                    first_let = false;
                } else {
                    out.push_str(&format!("           {post_name} == {tla_expr}\n"));
                }
            }
            out.push_str("       IN ");

            // Assign do-action results
            let mut first_conj = true;
            for (var, _) in &do_assignments {
                let post_name = format!("{var}_post");
                if first_conj {
                    out.push_str(&format!("/\\ {var}' = {post_name}\n"));
                    first_conj = false;
                } else {
                    out.push_str(&format!("          /\\ {var}' = {post_name}\n"));
                }
            }

            // Transition logic using post values
            render_transition_logic(&mut out, sm, &conditional, &unconditional,
                                    &do_modified, &all_branch_modified, "          ", true);
        } else if !do_assignments.is_empty() {
            // Do-actions without LET (no guard conflict)
            for (var, expr) in &do_assignments {
                let tla_expr = tla_expr::sysml_expr_to_tla(expr);
                out.push_str(&format!("    /\\ {var}' = {tla_expr}\n"));
            }
            // Transition logic
            render_transition_logic(&mut out, sm, &conditional, &unconditional,
                                    &do_modified, &all_branch_modified, "    ", false);
        } else {
            // No do-actions, just transitions
            render_transition_logic(&mut out, sm, &conditional, &unconditional,
                                    &do_modified, &all_branch_modified, "    ", false);
        }

        // Outer UNCHANGED
        if !unchanged_vars.is_empty() {
            out.push_str(&format!("    /\\ UNCHANGED <<{}>>\n", unchanged_vars.join(", ")));
        }

        out.push('\n');
    }

    out
}

/// Render transition logic (IF/THEN/ELSE chains or simple state assignment).
fn render_transition_logic(
    out: &mut String,
    sm: &StateMachine,
    conditional: &[&&Transition],
    unconditional: &[&&Transition],
    do_modified: &BTreeSet<String>,
    all_branch_modified: &BTreeSet<String>,
    indent: &str,
    use_post_names: bool,
) {
    if conditional.is_empty() && unconditional.len() == 1 {
        // Simple unconditional transition
        let t = unconditional[0];
        let entry_assigns = collect_entry_assignments_for_target(sm, &t.to_state);
        out.push_str(&format!("{indent}/\\ state' = \"{}\"\n", t.to_state));
        for (var, expr) in &entry_assigns {
            if !do_modified.contains(var) {
                let tla_expr = tla_expr::sysml_expr_to_tla(expr);
                out.push_str(&format!("{indent}/\\ {var}' = {tla_expr}\n"));
            }
        }
        // Variables modified in branch but not do or entry → stay same
        let branch_entry_vars: BTreeSet<String> = entry_assigns.iter()
            .map(|(v, _)| v.clone())
            .collect();
        for var in all_branch_modified {
            if !do_modified.contains(var) && !branch_entry_vars.contains(var) && var != "state" {
                out.push_str(&format!("{indent}/\\ {var}' = {var}\n"));
            }
        }
    } else if !conditional.is_empty() {
        // IF/ELSE IF/ELSE chain
        out.push_str(&format!("{indent}/\\ "));

        for (i, t) in conditional.iter().enumerate() {
            let cond = t.condition.as_ref().expect("conditional has condition");
            let mut tla_cond = tla_expr::sysml_condition_to_tla(cond);
            if use_post_names {
                tla_cond = replace_with_post_names(&tla_cond, do_modified);
            }

            if i == 0 {
                out.push_str(&format!("IF {tla_cond}\n"));
            } else {
                out.push_str(&format!("{indent}   ELSE IF {tla_cond}\n"));
            }

            let entry_assigns = collect_entry_assignments_for_target(sm, &t.to_state);
            out.push_str(&format!("{indent}   THEN /\\ state' = \"{}\"\n", t.to_state));
            for (var, expr) in &entry_assigns {
                if !do_modified.contains(var) {
                    let tla_expr = tla_expr::sysml_expr_to_tla(expr);
                    out.push_str(&format!("{indent}        /\\ {var}' = {tla_expr}\n"));
                }
            }
            // Variables modified in other branches but not this one
            emit_branch_unchanged(out, sm, all_branch_modified, do_modified, &t.to_state, indent);
        }

        // ELSE clause
        if !unconditional.is_empty() {
            let t = unconditional[0];
            let entry_assigns = collect_entry_assignments_for_target(sm, &t.to_state);
            out.push_str(&format!("{indent}   ELSE /\\ state' = \"{}\"\n", t.to_state));
            for (var, expr) in &entry_assigns {
                if !do_modified.contains(var) {
                    let tla_expr = tla_expr::sysml_expr_to_tla(expr);
                    out.push_str(&format!("{indent}        /\\ {var}' = {tla_expr}\n"));
                }
            }
            emit_branch_unchanged(out, sm, all_branch_modified, do_modified, &t.to_state, indent);
        } else {
            // No unconditional fallback → stay in same state
            out.push_str(&format!("{indent}   ELSE /\\ state' = state\n"));
            // All branch-modified vars stay unchanged
            for var in all_branch_modified {
                if !do_modified.contains(var) {
                    out.push_str(&format!("{indent}        /\\ {var}' = {var}\n"));
                }
            }
        }
    }
}

/// Emit UNCHANGED-equivalent primed assignments for variables modified in other
/// branches but not in the current target's entry actions.
fn emit_branch_unchanged(
    out: &mut String,
    sm: &StateMachine,
    all_branch_modified: &BTreeSet<String>,
    do_modified: &BTreeSet<String>,
    target_state: &str,
    indent: &str,
) {
    let entry_assigns = collect_entry_assignments_for_target(sm, target_state);
    let branch_vars: BTreeSet<String> = entry_assigns.iter()
        .map(|(v, _)| v.clone())
        .collect();
    for var in all_branch_modified {
        if !do_modified.contains(var) && !branch_vars.contains(var) {
            out.push_str(&format!("{indent}        /\\ {var}' = {var}\n"));
        }
    }
}

/// Replace variable names with their _post versions in a condition string.
fn replace_with_post_names(expr: &str, do_modified: &BTreeSet<String>) -> String {
    let mut result = expr.to_string();
    // Sort by length descending to avoid partial replacements
    let mut vars: Vec<&String> = do_modified.iter().collect();
    vars.sort_by(|a, b| b.len().cmp(&a.len()));
    for var in vars {
        let post_name = format!("{var}_post");
        result = word_replace(&result, var, &post_name);
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

/// Collect do-action assignments for a state.
fn collect_do_assignments(state: &State) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for action in &state.do_actions {
        if let Some(body) = &action.body {
            result.extend(tla_expr::parse_assignments(body));
        }
    }
    result
}

/// Collect entry-action assignments for a target state.
fn collect_entry_assignments_for_target(sm: &StateMachine, target_name: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    if let Some(target) = sm.states.iter().find(|s| s.name == target_name) {
        for action in &target.entry_actions {
            if let Some(body) = &action.body {
                result.extend(tla_expr::parse_assignments(body));
            }
        }
    }
    result
}

/// Render the .cfg file for TLC.
fn render_cfg_file(part: &PartDef, info: &AttrInfo) -> String {
    let mut out = String::new();
    out.push_str("SPECIFICATION Spec\n");

    if !info.constants.is_empty() {
        out.push_str("CONSTANTS\n");
        for c in &info.constants {
            let val = match &c.default {
                Some(d) => tla_expr::sysml_expr_to_tla(d),
                None => "0".to_string(),
            };
            out.push_str(&format!("    {} = {}\n", c.name, val));
        }
    }

    // State constraint if there are integer variables
    let has_int_vars = info.state_vars.iter()
        .any(|v| v.typ == "Integer" || v.typ == "Real");
    if has_int_vars {
        out.push_str("CONSTRAINT StateConstraint\n");
    }

    out.push_str("INVARIANTS\n");
    out.push_str("    TypeInvariant\n");
    out.push_str("    Safety\n");

    let has_env = !info.inputs.is_empty();

    // Determine if actor cycles (for liveness)
    let cycles = part.state_machine.as_ref()
        .map_or(false, |sm| all_states_have_transitions(sm));

    // Liveness property
    let has_liveness = !has_env || (has_env && cycles);
    if has_liveness {
        out.push_str("PROPERTIES\n");
        out.push_str("    Liveness\n");
    }

    out
}
