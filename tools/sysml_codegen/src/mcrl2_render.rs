//! mCRL2 specification generation from `SysML` AST.
//!
//! Generates `.mcrl2` and `.mcf` files directly from the `SysML` AST (Package).
//! Each `PartDef` with a state machine produces one mCRL2 spec plus
//! property files for deadlock freedom and liveness.

use crate::ast::{Package, PartDef, State, StateMachine, Transition};
use crate::buf;
use crate::mcrl2_expr;
use std::collections::BTreeSet;

/// `(property_name, mcf_content)`
pub type Mcrl2PropertyFile = (String, String);
/// `(part_name, mcrl2_content, property_files)`
pub type Mcrl2PartOutput = (String, String, Vec<Mcrl2PropertyFile>);

pub(crate) struct TransitionAssigns<'a> {
    pub(crate) during: &'a [(String, String)],
    pub(crate) leaving: &'a [(String, String)],
    pub(crate) stepping: &'a [(String, String)],
    pub(crate) entering: &'a [(String, String)],
}

enum RecursiveTail {
    None,
    Elapsed,
    ElapsedAndPhase(String),
}

struct RecursiveArgBuild<'a> {
    state_vars: &'a [VarInfo],
    target_state: &'a str,
    assigns: TransitionAssigns<'a>,
    tail: RecursiveTail,
}

fn transition_args<'a>(
    info: &'a AttrInfo,
    target_state: &'a str,
    assigns: TransitionAssigns<'a>,
    tail: RecursiveTail,
) -> String {
    build_recursive_args(RecursiveArgBuild {
        state_vars: &info.state_vars,
        target_state,
        assigns,
        tail,
    })
}

/// Render mCRL2 specs for all parts with state machines.
pub fn render_mcrl2(package: &Package) -> Vec<Mcrl2PartOutput> {
    let mut results = Vec::new();
    for part in &package.parts {
        if let Some(sm) = &part.state_machine {
            let info = classify_attributes(part, sm);
            let mcrl2 = render_mcrl2_module(part, sm, &info);
            let props = render_mcf_properties(part, sm);
            results.push((part.name.clone(), mcrl2, props));
        }
    }
    results
}

/// Information about an input variable extracted from `accept` transitions.
struct InputInfo {
    name: String,
    typ: String,
}

/// Classification of part attributes for mCRL2 generation.
struct AttrInfo {
    /// Constants: never assigned in any action body, not accept inputs.
    constants: Vec<ConstantInfo>,
    /// Input variables: appear in `accept` transitions (ordered).
    inputs: Vec<InputInfo>,
    /// Set of input variable names for fast containment checks.
    input_names: BTreeSet<String>,
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
        for action in state
            .entry_actions
            .iter()
            .chain(state.do_actions.iter())
            .chain(state.exit_actions.iter())
        {
            if let Some(body) = &action.body {
                for (var, _) in mcrl2_expr::parse_assignments(body) {
                    assigned.insert(var);
                }
            }
        }
    }
    for t in &sm.transitions {
        for action in &t.actions {
            if let Some(body) = &action.body {
                for (var, _) in mcrl2_expr::parse_assignments(body) {
                    assigned.insert(var);
                }
            }
        }
    }

    // Collect accept (input) variables — use accept_var field, not condition parsing
    let mut input_names: BTreeSet<String> = BTreeSet::new();
    for t in &sm.transitions {
        if let Some(var) = &t.accept_var {
            let var_name = extract_accept_var_name(var.trim());
            input_names.insert(var_name);
        } else if t.is_accept {
            // Fallback for backward compat: parse condition
            if let Some(cond) = &t.condition {
                for word in cond.split(" and ") {
                    let var_name = extract_accept_var_name(word.trim());
                    input_names.insert(var_name);
                }
            }
        }
    }

    let mut constants = Vec::new();
    let mut state_vars = Vec::new();

    for attr in &part.attributes {
        let name = &attr.name;
        if input_names.contains(name) || assigned.contains(name) {
            state_vars.push(VarInfo {
                name: name.clone(),
                typ: attr.typ.clone(),
                default: mcrl2_default(&attr.typ, attr.default.as_deref()),
            });
        } else {
            constants.push(ConstantInfo {
                name: name.clone(),
                default: attr.default.clone(),
            });
        }
    }

    // Build InputInfo list in deterministic order
    let inputs: Vec<InputInfo> = input_names
        .iter()
        .map(|name| {
            let typ = part
                .attributes
                .iter()
                .find(|a| &a.name == name)
                .map_or_else(|| "Boolean".to_string(), |a| a.typ.clone());
            InputInfo {
                name: name.clone(),
                typ,
            }
        })
        .collect();

    AttrInfo {
        constants,
        inputs,
        input_names,
        state_vars,
    }
}

/// Get mCRL2 default value for a `SysML` type.
fn mcrl2_default(typ: &str, default: Option<&str>) -> String {
    if let Some(val) = default {
        return mcrl2_expr::sysml_expr_to_mcrl2(val);
    }
    match typ {
        "Boolean" => "false".to_string(),
        _ => "0".to_string(),
    }
}

/// Map `SysML` type to mCRL2 sort name.
fn mcrl2_sort(typ: &str) -> &'static str {
    match typ {
        "Boolean" => "Bool",
        "Real" => "Int",
        _ => "Nat",
    }
}

/// Extract the variable name from an accept condition.
fn extract_accept_var_name(cond: &str) -> String {
    let operators = [">=", "<=", "!=", "==", ">", "<"];
    for op in &operators {
        if let Some(pos) = cond.find(op) {
            return cond[..pos].trim().to_string();
        }
    }
    cond.to_string()
}

/// Render the complete mCRL2 module for a part.
fn render_mcrl2_module(part: &PartDef, sm: &StateMachine, info: &AttrInfo) -> String {
    let mut out = String::new();
    let name = &part.name;

    buf::append(&mut out, format_args!("% mCRL2 specification for {name}\n"));
    out.push_str("% Generated from SysML model\n\n");

    let state_names: Vec<&str> = sm.states.iter().map(|s| s.name.as_str()).collect();
    buf::append(
        &mut out,
        format_args!("sort State = struct {};\n\n", state_names.join(" | ")),
    );

    mcrl2_write_constants(&mut out, part, info);
    out.push_str("act step: State # State;\n");
    mcrl2_write_env_action_decls(&mut out, part, info);

    let mut params: Vec<String> = vec!["s: State".to_string()];
    for v in &info.state_vars {
        params.push(format!("{}: {}", v.name, mcrl2_sort(&v.typ)));
    }
    buf::append(
        &mut out,
        format_args!("proc {}({}) =\n", name, params.join(", ")),
    );

    let mut first_choice = true;
    Mcrl2StateChoices {
        out: &mut out,
        first_choice: &mut first_choice,
        proc_name: name,
        sm,
        info,
        guard_lead: "",
        tail: Mcrl2TransitionTail::None,
        self_loop: Mcrl2SelfLoopKind::Basic,
    }
    .write_all();
    mcrl2_write_basic_env_in_process(&mut out, part, info, name, &mut first_choice);

    out.push_str("  ;\n\n");

    let initial_state = &sm.states[0].name;
    let init_args: Vec<String> = std::iter::once(initial_state.clone())
        .chain(info.state_vars.iter().map(|v| v.default.clone()))
        .collect();
    buf::append(
        &mut out,
        format_args!("init {}({});\n", name, init_args.join(", ")),
    );

    out
}

/// Build recursive call arguments for a transition to a target state.
fn build_recursive_args(params: RecursiveArgBuild<'_>) -> String {
    let RecursiveArgBuild {
        state_vars,
        target_state,
        assigns,
        tail,
    } = params;
    let TransitionAssigns {
        during,
        leaving,
        stepping,
        entering,
    } = assigns;

    let mut args: Vec<String> = vec![target_state.to_string()];

    for v in state_vars {
        if let Some((_, expr)) = entering.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else if let Some((_, expr)) = stepping.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else if let Some((_, expr)) = leaving.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else if let Some((_, expr)) = during.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else {
            args.push(v.name.clone());
        }
    }

    match tail {
        RecursiveTail::None => {}
        RecursiveTail::Elapsed => args.push("elapsed".to_string()),
        RecursiveTail::ElapsedAndPhase(phase) => {
            args.push("elapsed".to_string());
            args.push(phase);
        }
    }

    args.join(", ")
}

/// Build recursive call arguments for a self-loop (no state change).
fn build_self_loop_args(
    _sm: &StateMachine,
    state_vars: &[VarInfo],
    _input_names: &BTreeSet<String>,
    current_state: &str,
    during: &[(String, String)],
    leaving: &[(String, String)],
) -> String {
    let mut args: Vec<String> = vec![current_state.to_string()];

    for v in state_vars {
        if let Some((_, expr)) = leaving.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else if let Some((_, expr)) = during.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else {
            args.push(v.name.clone());
        }
    }

    args.join(", ")
}

/// Build recursive call arguments for an env input action (single variable changes).
fn build_env_args(state_vars: &[VarInfo], input_name: &str, bound_var: &str) -> String {
    let mut args: Vec<String> = vec!["s".to_string()];

    for v in state_vars {
        if v.name == input_name {
            args.push(bound_var.to_string());
        } else {
            args.push(v.name.clone());
        }
    }

    args.join(", ")
}

/// Build recursive call arguments for a grouped env input action.
fn build_group_env_args(state_vars: &[VarInfo], members: &[String]) -> String {
    let mut args: Vec<String> = vec!["s".to_string()];

    for v in state_vars {
        if let Some(pos) = members.iter().position(|m| *m == v.name) {
            args.push(format!("{}_v", members[pos]));
        } else {
            args.push(v.name.clone());
        }
    }

    args.join(", ")
}

/// Render .mcf property files for a part.
fn render_mcf_properties(part: &PartDef, sm: &StateMachine) -> Vec<(String, String)> {
    let mut props = Vec::new();

    // Deadlock freedom: every reachable state has at least one successor
    let deadlock_name = format!("{}_deadlock_freedom", part.name);
    let deadlock_mcf = "[true*]<true>true\n".to_string();
    props.push((deadlock_name, deadlock_mcf));

    // Liveness: the initial state is always eventually reachable
    let initial_state = &sm.states[0].name;
    let liveness_name = format!("{}_liveness", part.name);

    // Build: nu X. mu Y. (<step(S1, Init)>X || ... || (exists/forall over step only))
    // Uses step-only quantification to ignore env action stutter.
    let source_states: Vec<&str> = sm
        .states
        .iter()
        .filter(|s| sm.transitions.iter().any(|t| t.from_state == s.name))
        .map(|s| s.name.as_str())
        .collect();

    let step_disjuncts: Vec<String> = source_states
        .iter()
        .map(|s| format!("<step({s}, {initial_state})>X"))
        .collect();

    let liveness_mcf = if step_disjuncts.is_empty() {
        "true\n".to_string()
    } else {
        format!("nu X. mu Y. ({} || ((exists s1,s2: State . <step(s1, s2)>Y) && (forall s1,s2: State . [step(s1, s2)]Y)))\n",
            step_disjuncts.join(" || "))
    };

    props.push((liveness_name, liveness_mcf));

    props
}

/// Collect do-action assignments for a state.
fn collect_do_assignments(state: &State) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for action in &state.do_actions {
        if let Some(body) = &action.body {
            result.extend(mcrl2_expr::parse_assignments(body));
        }
    }
    result
}

/// Collect entry-action assignments for a target state.
fn collect_entry_assignments_for_target(
    sm: &StateMachine,
    target_name: &str,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    if let Some(target) = sm.states.iter().find(|s| s.name == target_name) {
        for action in &target.entry_actions {
            if let Some(body) = &action.body {
                result.extend(mcrl2_expr::parse_assignments(body));
            }
        }
    }
    result
}

/// Collect exit-action assignments for a source state.
fn collect_exit_assignments(state: &State) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for action in &state.exit_actions {
        if let Some(body) = &action.body {
            result.extend(mcrl2_expr::parse_assignments(body));
        }
    }
    result
}

/// Collect transition-action assignments for a transition.
fn collect_transition_action_assignments(t: &Transition) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for action in &t.actions {
        if let Some(body) = &action.body {
            result.extend(mcrl2_expr::parse_assignments(body));
        }
    }
    result
}

enum Mcrl2SelfLoopKind {
    Basic,
    Timed,
    Wcet,
}

enum Mcrl2TransitionTail {
    None,
    Elapsed,
    Wcet,
}

struct Mcrl2StateChoices<'a> {
    out: &'a mut String,
    first_choice: &'a mut bool,
    proc_name: &'a str,
    sm: &'a StateMachine,
    info: &'a AttrInfo,
    guard_lead: &'static str,
    tail: Mcrl2TransitionTail,
    self_loop: Mcrl2SelfLoopKind,
}

impl Mcrl2StateChoices<'_> {
    fn choice_prefix(&mut self) -> &'static str {
        let prefix = if *self.first_choice { "  " } else { "  + " };
        *self.first_choice = false;
        prefix
    }

    fn transition_tail(&self, target: &str) -> RecursiveTail {
        match self.tail {
            Mcrl2TransitionTail::None => RecursiveTail::None,
            Mcrl2TransitionTail::Elapsed => RecursiveTail::Elapsed,
            Mcrl2TransitionTail::Wcet => RecursiveTail::ElapsedAndPhase(format!("PHASE_{target}")),
        }
    }

    fn self_loop_args(
        &self,
        state_name: &str,
        during: &[(String, String)],
        leaving: &[(String, String)],
    ) -> String {
        match self.self_loop {
            Mcrl2SelfLoopKind::Basic => build_self_loop_args(
                self.sm,
                &self.info.state_vars,
                &self.info.input_names,
                state_name,
                during,
                leaving,
            ),
            Mcrl2SelfLoopKind::Timed => build_timed_self_loop_args(
                self.sm,
                &self.info.state_vars,
                &self.info.input_names,
                state_name,
                during,
                leaving,
            ),
            Mcrl2SelfLoopKind::Wcet => build_wcet_self_loop_args(
                self.sm,
                &self.info.state_vars,
                &self.info.input_names,
                state_name,
                during,
                leaving,
            ),
        }
    }

    fn write_all(&mut self) {
        for state in &self.sm.states {
            self.write_state(state);
        }
    }

    fn write_state(&mut self, state: &State) {
        let transitions: Vec<&Transition> = self
            .sm
            .transitions
            .iter()
            .filter(|t| t.from_state == state.name)
            .collect();
        if transitions.is_empty() {
            return;
        }

        let do_assignments = collect_do_assignments(state);
        let exit_assignments = collect_exit_assignments(state);
        let conditional: Vec<&&Transition> = transitions
            .iter()
            .filter(|t| t.condition.is_some())
            .collect();
        let unconditional: Vec<&&Transition> = transitions
            .iter()
            .filter(|t| t.condition.is_none())
            .collect();

        self.write_conditional(state, &do_assignments, &exit_assignments, &conditional);
        self.write_unconditional(
            state,
            &do_assignments,
            &exit_assignments,
            &conditional,
            &unconditional,
        );
        self.write_self_loop(
            state,
            &do_assignments,
            &exit_assignments,
            &conditional,
            &unconditional,
        );
    }

    fn write_conditional(
        &mut self,
        state: &State,
        during: &[(String, String)],
        leaving: &[(String, String)],
        conditional: &[&&Transition],
    ) {
        for t in conditional {
            let cond = t.condition.as_ref().expect("conditional has condition");
            let mcrl2_cond = mcrl2_guard_with_do_substitution(cond, during);
            let prefix = self.choice_prefix();
            let target = &t.to_state;
            let args = transition_args(
                self.info,
                target,
                TransitionAssigns {
                    during,
                    leaving,
                    stepping: &collect_transition_action_assignments(t),
                    entering: &collect_entry_assignments_for_target(self.sm, target),
                },
                self.transition_tail(target),
            );
            buf::append(
                self.out,
                format_args!(
                    "{prefix}({guard}s == {sname} && {mcrl2_cond}) -> step({sname}, {target}) . {proc}({args})\n",
                    guard = self.guard_lead,
                    sname = state.name,
                    proc = self.proc_name
                ),
            );
        }
    }

    fn write_unconditional(
        &mut self,
        state: &State,
        during: &[(String, String)],
        leaving: &[(String, String)],
        conditional: &[&&Transition],
        unconditional: &[&&Transition],
    ) {
        if unconditional.len() != 1 {
            return;
        }
        let t = unconditional[0];
        let target = &t.to_state;
        let prefix = self.choice_prefix();
        let assigns = TransitionAssigns {
            during,
            leaving,
            stepping: &collect_transition_action_assignments(t),
            entering: &collect_entry_assignments_for_target(self.sm, target),
        };
        let args = transition_args(self.info, target, assigns, self.transition_tail(target));

        if conditional.is_empty() {
            buf::append(
                self.out,
                format_args!(
                    "{prefix}({guard}s == {sname}) -> step({sname}, {target}) . {proc}({args})\n",
                    guard = self.guard_lead,
                    sname = state.name,
                    proc = self.proc_name
                ),
            );
            return;
        }

        let neg = mcrl2_negated_conditional_guards(conditional, during).join(" && ");
        buf::append(
            self.out,
            format_args!(
                "{prefix}({guard}s == {sname} && {neg}) -> step({sname}, {target}) . {proc}({args})\n",
                guard = self.guard_lead,
                sname = state.name,
                proc = self.proc_name
            ),
        );
    }

    fn write_self_loop(
        &mut self,
        state: &State,
        during: &[(String, String)],
        leaving: &[(String, String)],
        conditional: &[&&Transition],
        unconditional: &[&&Transition],
    ) {
        if !unconditional.is_empty() || conditional.is_empty() {
            return;
        }
        let neg = mcrl2_negated_conditional_guards(conditional, during).join(" && ");
        let prefix = self.choice_prefix();
        let args = self.self_loop_args(&state.name, during, leaving);
        buf::append(
            self.out,
            format_args!(
                "{prefix}({guard}s == {sname} && {neg}) -> step({sname}, {sname}) . {proc}({args})\n",
                guard = self.guard_lead,
                sname = state.name,
                proc = self.proc_name
            ),
        );
    }
}

fn mcrl2_guard_with_do_substitution(cond: &str, during: &[(String, String)]) -> String {
    let mut mcrl2_cond = mcrl2_expr::sysml_condition_to_mcrl2(cond);
    for (var, expr) in during {
        let mcrl2_val = mcrl2_expr::sysml_expr_to_mcrl2(expr);
        mcrl2_cond = word_replace(&mcrl2_cond, var, &mcrl2_val);
    }
    mcrl2_cond
}

fn mcrl2_negated_conditional_guards(
    conditional: &[&&Transition],
    during: &[(String, String)],
) -> Vec<String> {
    conditional
        .iter()
        .map(|ct| {
            let cond = ct.condition.as_ref().expect("conditional has condition");
            format!("!({})", mcrl2_guard_with_do_substitution(cond, during))
        })
        .collect()
}

fn mcrl2_write_constants(out: &mut String, part: &PartDef, info: &AttrInfo) {
    if info.constants.is_empty() {
        return;
    }
    for c in &info.constants {
        let sort = part
            .attributes
            .iter()
            .find(|a| a.name == c.name)
            .map_or("Int", |a| mcrl2_sort(&a.typ));
        let val = match &c.default {
            Some(d) => mcrl2_expr::sysml_expr_to_mcrl2(d),
            None => "0".to_string(),
        };
        buf::append(out, format_args!("map {}: {};\n", c.name, sort));
        buf::append(out, format_args!("eqn {} = {};\n", c.name, val));
    }
    out.push('\n');
}

fn mcrl2_ungrouped_inputs<'a>(part: &PartDef, info: &'a AttrInfo) -> Vec<&'a InputInfo> {
    let grouped_names: BTreeSet<String> = part
        .input_groups
        .iter()
        .flat_map(|g| g.members.iter().cloned())
        .collect();
    info.inputs
        .iter()
        .filter(|i| !grouped_names.contains(&i.name))
        .collect()
}

fn mcrl2_write_env_action_decls(out: &mut String, part: &PartDef, info: &AttrInfo) {
    let ungrouped = mcrl2_ungrouped_inputs(part, info);
    for input in &ungrouped {
        let sort = mcrl2_sort(&input.typ);
        buf::append(out, format_args!("    env_{}: {};\n", input.name, sort));
    }
    for group in &part.input_groups {
        let sorts: Vec<String> = group
            .members
            .iter()
            .map(|m| {
                info.inputs
                    .iter()
                    .find(|i| i.name == *m)
                    .map_or("Bool", |i| mcrl2_sort(&i.typ))
                    .to_string()
            })
            .collect();
        buf::append(
            out,
            format_args!("    env_{}: {};\n", group.name, sorts.join(" # ")),
        );
    }
    out.push('\n');
}

fn mcrl2_write_basic_env_in_process(
    out: &mut String,
    part: &PartDef,
    info: &AttrInfo,
    proc_name: &str,
    first_choice: &mut bool,
) {
    if info.inputs.is_empty() {
        return;
    }
    let ungrouped = mcrl2_ungrouped_inputs(part, info);
    for input in &ungrouped {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sort = mcrl2_sort(&input.typ);
        let bound_var = format!("{}_v", input.name);
        let args = build_env_args(&info.state_vars, &input.name, &bound_var);
        buf::append(
            out,
            format_args!(
                "{prefix}sum {bound_var}: {sort} . env_{iname}({bound_var}) . {proc_name}({args})\n",
                iname = input.name
            ),
        );
    }
    for group in &part.input_groups {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sum_vars: Vec<String> = group
            .members
            .iter()
            .map(|m| {
                let sort = info
                    .inputs
                    .iter()
                    .find(|i| i.name == *m)
                    .map_or("Bool", |i| mcrl2_sort(&i.typ));
                format!("{m}_v: {sort}")
            })
            .collect();
        let action_args: Vec<String> = group.members.iter().map(|m| format!("{m}_v")).collect();
        let args = build_group_env_args(&info.state_vars, &group.members);
        buf::append(
            out,
            format_args!(
                "{prefix}sum {} . env_{}({}) . {proc_name}({args})\n",
                sum_vars.join(", "),
                group.name,
                action_args.join(", ")
            ),
        );
    }
}

// ============================================================================
// Timed mCRL2 Specification Generation
// ============================================================================

/// Timing attributes extracted from a `PartDef`.
pub struct TimingInfo {
    pub execution_period_ms: Option<u64>,
    pub max_execution_time_us: Option<u64>,
    pub max_jitter_us: Option<u64>,
    pub debounce_period_ms: Option<u64>,
}

/// Extract timing attributes from a part definition.
pub fn extract_timing_info(part: &PartDef) -> TimingInfo {
    let get = |name: &str| -> Option<u64> {
        part.attributes
            .iter()
            .find(|a| a.name == name)
            .and_then(|a| a.default.as_ref())
            .and_then(|d| d.trim().parse::<u64>().ok())
    };
    TimingInfo {
        execution_period_ms: get("execution_period_ms"),
        max_execution_time_us: get("max_execution_time_us"),
        max_jitter_us: get("max_jitter_us"),
        debounce_period_ms: get("debounce_period_ms"),
    }
}

/// Returns true if the part has any timing attributes worth modeling.
pub fn has_timing(timing: &TimingInfo) -> bool {
    timing.execution_period_ms.is_some()
        || timing.max_execution_time_us.is_some()
        || timing.debounce_period_ms.is_some()
}

/// Compute the time step (in µs) as the GCD of all timing values, clamped to a practical minimum.
pub fn compute_time_step(timing: &TimingInfo) -> u64 {
    let mut values: Vec<u64> = Vec::new();
    if let Some(p) = timing.execution_period_ms {
        values.push(p * 1000); // convert ms to µs
    }
    if let Some(d) = timing.max_execution_time_us {
        values.push(d);
    }
    if let Some(j) = timing.max_jitter_us {
        values.push(j);
    }
    if let Some(db) = timing.debounce_period_ms {
        values.push(db * 1000); // convert ms to µs
    }

    if values.is_empty() {
        return 1000; // default 1ms
    }

    let mut g = values[0];
    for &v in &values[1..] {
        g = gcd(g, v);
    }
    // Clamp to practical minimum (avoid enormous tick counts)
    g.max(1)
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Is this a periodic actor (has `execution_period_ms`)?
fn is_periodic(timing: &TimingInfo) -> bool {
    timing.execution_period_ms.is_some()
}

/// Is this an event-driven actor with debounce?
fn is_debounce(timing: &TimingInfo) -> bool {
    timing.debounce_period_ms.is_some() && timing.execution_period_ms.is_none()
}

/// Render timed mCRL2 specs for all parts with state machines and timing attributes.
/// Returns `(part_name_timed, mcrl2_content, [(prop_name, mcf_content)])` per part.
pub fn render_timed_mcrl2(package: &Package) -> Vec<Mcrl2PartOutput> {
    let mut results = Vec::new();
    for part in &package.parts {
        if let Some(sm) = &part.state_machine {
            let timing = extract_timing_info(part);
            if !has_timing(&timing) {
                continue;
            }
            let info = classify_attributes(part, sm);
            let (mcrl2, props) = if has_wcet_data(sm) {
                let mcrl2 = render_wcet_timed_mcrl2_module(part, sm, &info, &timing);
                let props = render_timed_mcf_properties(part, sm, &timing);
                (mcrl2, props)
            } else {
                let mcrl2 = render_timed_mcrl2_module(part, sm, &info, &timing);
                let props = render_timed_mcf_properties(part, sm, &timing);
                (mcrl2, props)
            };
            let timed_name = format!("{}_timed", part.name);
            results.push((timed_name, mcrl2, props));
        }
    }
    results
}

struct TimedTickConstants {
    time_step: u64,
    period_ticks: u64,
    jitter_ticks: u64,
    debounce_ticks: u64,
    max_elapsed: u64,
}

fn timed_tick_constants(timing: &TimingInfo) -> TimedTickConstants {
    let time_step = compute_time_step(timing);
    let period_ticks = timing
        .execution_period_ms
        .map_or(0, |p| p * 1000 / time_step);
    let jitter_ticks = timing.max_jitter_us.map_or(0, |j| j / time_step);
    let debounce_ticks = timing
        .debounce_period_ms
        .map_or(0, |d| d * 1000 / time_step);
    let max_elapsed = if is_periodic(timing) {
        period_ticks + jitter_ticks + 1
    } else {
        debounce_ticks + 1
    };
    TimedTickConstants {
        time_step,
        period_ticks,
        jitter_ticks,
        debounce_ticks,
        max_elapsed,
    }
}

fn timed_write_timing_maps(out: &mut String, timing: &TimingInfo, ticks: &TimedTickConstants) {
    out.push_str("% Timing constants (in ticks)\n");
    buf::append(
        out,
        format_args!(
            "map TIME_STEP: Nat;\neqn TIME_STEP = {};\n",
            ticks.time_step
        ),
    );
    if is_periodic(timing) {
        buf::append(
            out,
            format_args!(
                "map PERIOD_TICKS: Nat;\neqn PERIOD_TICKS = {};\n",
                ticks.period_ticks
            ),
        );
    }
    if ticks.jitter_ticks > 0 {
        buf::append(
            out,
            format_args!(
                "map JITTER_TICKS: Nat;\neqn JITTER_TICKS = {};\n",
                ticks.jitter_ticks
            ),
        );
    }
    if is_debounce(timing) {
        buf::append(
            out,
            format_args!(
                "map DEBOUNCE_TICKS: Nat;\neqn DEBOUNCE_TICKS = {};\n",
                ticks.debounce_ticks
            ),
        );
    }
    buf::append(
        out,
        format_args!(
            "map MAX_ELAPSED: Nat;\neqn MAX_ELAPSED = {};\n",
            ticks.max_elapsed
        ),
    );
    out.push('\n');
}

fn timed_write_process_tail(
    out: &mut String,
    first_choice: &mut bool,
    proc_name: &str,
    part: &PartDef,
    info: &AttrInfo,
    timing: &TimingInfo,
    ticks: &TimedTickConstants,
) {
    if is_periodic(timing) {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let vars = build_timed_passthrough_vars(&info.state_vars);
        if ticks.jitter_ticks > 0 {
            buf::append(out, format_args!(
                "{prefix}(elapsed >= PERIOD_TICKS - JITTER_TICKS && elapsed <= PERIOD_TICKS + JITTER_TICKS) -> activate . {proc_name}(s, {vars}, 0)\n"));
        } else {
            buf::append(
                out,
                format_args!(
                    "{prefix}(elapsed >= PERIOD_TICKS) -> activate . {proc_name}(s, {vars}, 0)\n"
                ),
            );
        }
    }

    let ungrouped = mcrl2_ungrouped_inputs(part, info);
    if is_debounce(timing) {
        timed_write_debounce_env(out, first_choice, proc_name, part, info, &ungrouped);
    } else {
        timed_write_plain_env(out, first_choice, proc_name, part, info, &ungrouped);
    }

    let prefix = if *first_choice { "  " } else { "  + " };
    let tick_args = build_timed_simple_tick_args(&info.state_vars);
    buf::append(
        out,
        format_args!("{prefix}(elapsed < MAX_ELAPSED) -> tick . {proc_name}({tick_args})\n"),
    );
}

fn timed_write_debounce_env(
    out: &mut String,
    first_choice: &mut bool,
    proc_name: &str,
    part: &PartDef,
    info: &AttrInfo,
    ungrouped: &[&InputInfo],
) {
    for input in ungrouped {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sort = mcrl2_sort(&input.typ);
        let bound_var = format!("{}_v", input.name);
        let args = build_timed_env_args(&info.state_vars, &input.name, &bound_var);
        buf::append(out, format_args!(
            "{prefix}(elapsed >= DEBOUNCE_TICKS) -> sum {bound_var}: {sort} . env_{iname}({bound_var}) . {proc_name}({args})\n",
            iname = input.name));
    }
    timed_write_debounce_group_env(out, first_choice, proc_name, part, info);
}

fn timed_write_debounce_group_env(
    out: &mut String,
    first_choice: &mut bool,
    proc_name: &str,
    part: &PartDef,
    info: &AttrInfo,
) {
    for group in &part.input_groups {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sum_vars: Vec<String> = group
            .members
            .iter()
            .map(|m| {
                let sort = info
                    .inputs
                    .iter()
                    .find(|i| i.name == *m)
                    .map_or("Bool", |i| mcrl2_sort(&i.typ));
                format!("{m}_v: {sort}")
            })
            .collect();
        let action_args: Vec<String> = group.members.iter().map(|m| format!("{m}_v")).collect();
        let args = build_timed_group_env_args(&info.state_vars, &group.members);
        buf::append(
            out,
            format_args!(
                "{prefix}(elapsed >= DEBOUNCE_TICKS) -> sum {} . env_{}({}) . {proc_name}({args})\n",
                sum_vars.join(", "),
                group.name,
                action_args.join(", ")
            ),
        );
    }
}

fn timed_write_plain_env(
    out: &mut String,
    first_choice: &mut bool,
    proc_name: &str,
    part: &PartDef,
    info: &AttrInfo,
    ungrouped: &[&InputInfo],
) {
    for input in ungrouped {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sort = mcrl2_sort(&input.typ);
        let bound_var = format!("{}_v", input.name);
        let args = build_timed_env_args(&info.state_vars, &input.name, &bound_var);
        buf::append(
            out,
            format_args!(
                "{prefix}sum {bound_var}: {sort} . env_{iname}({bound_var}) . {proc_name}({args})\n",
                iname = input.name
            ),
        );
    }
    for group in &part.input_groups {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sum_vars: Vec<String> = group
            .members
            .iter()
            .map(|m| {
                let sort = info
                    .inputs
                    .iter()
                    .find(|i| i.name == *m)
                    .map_or("Bool", |i| mcrl2_sort(&i.typ));
                format!("{m}_v: {sort}")
            })
            .collect();
        let action_args: Vec<String> = group.members.iter().map(|m| format!("{m}_v")).collect();
        let args = build_timed_group_env_args(&info.state_vars, &group.members);
        buf::append(
            out,
            format_args!(
                "{prefix}sum {} . env_{}({}) . {proc_name}({args})\n",
                sum_vars.join(", "),
                group.name,
                action_args.join(", ")
            ),
        );
    }
}

/// Render the complete timed mCRL2 module for a part.
///
/// Steps are instantaneous (no deadline guard) — same logic as untimed model.
/// Tick only advances elapsed (do-actions fire on step transitions, not on tick).
/// This avoids the nondeterministic-choice problem where `deadline_miss` is always
/// reachable because the scheduler can choose tick over step.
fn render_timed_mcrl2_module(
    part: &PartDef,
    sm: &StateMachine,
    info: &AttrInfo,
    timing: &TimingInfo,
) -> String {
    let mut out = String::new();
    let name = &part.name;
    let ticks = timed_tick_constants(timing);

    buf::append(
        &mut out,
        format_args!("% Timed mCRL2 specification for {name}\n"),
    );
    out.push_str("% Generated from SysML model (discrete tick-based time)\n");
    buf::append(
        &mut out,
        format_args!("% Time step: {}us\n\n", ticks.time_step),
    );

    let state_names: Vec<&str> = sm.states.iter().map(|s| s.name.as_str()).collect();
    buf::append(
        &mut out,
        format_args!("sort State = struct {};\n\n", state_names.join(" | ")),
    );

    timed_write_timing_maps(&mut out, timing, &ticks);
    mcrl2_write_constants(&mut out, part, info);

    out.push_str("act step: State # State;\n");
    out.push_str("    tick;\n");
    if is_periodic(timing) {
        out.push_str("    activate;\n");
    }
    mcrl2_write_env_action_decls(&mut out, part, info);

    let mut params: Vec<String> = vec!["s: State".to_string()];
    for v in &info.state_vars {
        params.push(format!("{}: {}", v.name, mcrl2_sort(&v.typ)));
    }
    params.push("elapsed: Nat".to_string());
    buf::append(
        &mut out,
        format_args!("proc {}({}) =\n", name, params.join(", ")),
    );

    let initial_state = &sm.states[0].name;
    let mut first_choice = true;
    Mcrl2StateChoices {
        out: &mut out,
        first_choice: &mut first_choice,
        proc_name: name,
        sm,
        info,
        guard_lead: "",
        tail: Mcrl2TransitionTail::Elapsed,
        self_loop: Mcrl2SelfLoopKind::Timed,
    }
    .write_all();
    timed_write_process_tail(
        &mut out,
        &mut first_choice,
        name,
        part,
        info,
        timing,
        &ticks,
    );

    out.push_str("  ;\n\n");

    let init_args: Vec<String> = std::iter::once(initial_state.clone())
        .chain(info.state_vars.iter().map(|v| v.default.clone()))
        .chain(std::iter::once("0".to_string()))
        .collect();
    buf::append(
        &mut out,
        format_args!("init {}({});\n", name, init_args.join(", ")),
    );

    out
}

/// Build passthrough variable list (just names, no state, no elapsed).
fn build_timed_passthrough_vars(state_vars: &[VarInfo]) -> String {
    state_vars
        .iter()
        .map(|v| v.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Build recursive call arguments for a timed self-loop.
fn build_timed_self_loop_args(
    _sm: &StateMachine,
    state_vars: &[VarInfo],
    _input_names: &BTreeSet<String>,
    current_state: &str,
    during: &[(String, String)],
    leaving: &[(String, String)],
) -> String {
    let mut args: Vec<String> = vec![current_state.to_string()];

    for v in state_vars {
        if let Some((_, expr)) = leaving.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else if let Some((_, expr)) = during.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else {
            args.push(v.name.clone());
        }
    }

    args.push("elapsed".to_string());
    args.join(", ")
}

/// Build args for a tick action: state and vars unchanged, elapsed + 1.
/// Do-action effects are applied on step transitions, not on tick.
fn build_timed_simple_tick_args(state_vars: &[VarInfo]) -> String {
    let mut args: Vec<String> = vec!["s".to_string()];
    for v in state_vars {
        args.push(v.name.clone());
    }
    args.push("elapsed + 1".to_string());
    args.join(", ")
}

/// Build args for env input action in timed model (passes elapsed through).
fn build_timed_env_args(state_vars: &[VarInfo], input_name: &str, bound_var: &str) -> String {
    let mut args: Vec<String> = vec!["s".to_string()];
    for v in state_vars {
        if v.name == input_name {
            args.push(bound_var.to_string());
        } else {
            args.push(v.name.clone());
        }
    }
    // Reset elapsed on env input for debounce actors
    args.push("0".to_string());
    args.join(", ")
}

/// Build args for grouped env input action in timed model.
fn build_timed_group_env_args(state_vars: &[VarInfo], members: &[String]) -> String {
    let mut args: Vec<String> = vec!["s".to_string()];
    for v in state_vars {
        if let Some(pos) = members.iter().position(|m| *m == v.name) {
            args.push(format!("{}_v", members[pos]));
        } else {
            args.push(v.name.clone());
        }
    }
    args.push("0".to_string());
    args.join(", ")
}

/// Render timed .mcf property files for a part.
fn render_timed_mcf_properties(
    part: &PartDef,
    sm: &StateMachine,
    timing: &TimingInfo,
) -> Vec<(String, String)> {
    let mut props = Vec::new();

    // Timed deadlock freedom: every reachable state has at least one successor
    // (verifies the timed model doesn't introduce deadlocks from tick/activate/debounce)
    let deadlock_name = format!("{}_timed_deadlock_freedom", part.name);
    let deadlock_mcf = "[true*]<true>true\n".to_string();
    props.push((deadlock_name, deadlock_mcf));

    // Deadline property: no deadline misses (only for periodic actors with WCET data)
    if has_wcet_data(sm) && is_periodic(timing) {
        let deadline_name = format!("{}_timed_deadline", part.name);
        let deadline_mcf = "[true*][deadline_miss]false\n".to_string();
        props.push((deadline_name, deadline_mcf));
    }

    // Response property: after activation/env input, a step eventually occurs
    if is_periodic(timing) {
        let name = format!("{}_timed_response", part.name);
        // After activate, a step is always available (possibly after ticks)
        let mcf = "nu X. [activate](mu Y. ((exists s1,s2: State . <step(s1, s2)>true) && X) || <tick>Y) && [true]X\n".to_string();
        props.push((name, mcf));
    } else if is_debounce(timing) {
        let name = format!("{}_timed_response", part.name);
        // After any env input, a step is always available (possibly after ticks)
        // Use forall over the input parameter to match the parameterized action
        let input_name = sm
            .transitions
            .iter()
            .find_map(|t| t.accept_var.as_ref())
            .map_or_else(|| "x".to_string(), |s| s.trim().to_string());
        let input_sort = part
            .attributes
            .iter()
            .find(|a| a.name == input_name)
            .map_or("Bool", |a| mcrl2_sort(&a.typ));
        let mcf = format!(
            "nu X. (forall v: {input_sort} . [env_{input_name}(v)](mu Y. ((exists s1,s2: State . <step(s1, s2)>true) && X) || <tick>Y)) && [true]X\n"
        );
        props.push((name, mcf));
    }

    props
}

// ============================================================================
// Phase-Based WCET Timed mCRL2 Model
// ============================================================================

/// Returns true if any action in the state machine has a WCET annotation.
fn has_wcet_data(sm: &StateMachine) -> bool {
    for state in &sm.states {
        for action in state
            .entry_actions
            .iter()
            .chain(state.do_actions.iter())
            .chain(state.exit_actions.iter())
        {
            if action.wcet_us.is_some() {
                return true;
            }
        }
    }
    false
}

/// Compute the phase cost (in ticks) for a state: sum of WCET of entry + do actions,
/// converted via ceiling division: `ceil(wcet_us` / `time_step`).
fn compute_state_phase_ticks(state: &State, time_step: u64) -> u64 {
    let mut total_us: u64 = 0;
    for action in state.entry_actions.iter().chain(state.do_actions.iter()) {
        if let Some(wcet) = action.wcet_us {
            total_us += wcet;
        }
    }
    if total_us == 0 {
        return 0;
    }
    total_us.div_ceil(time_step)
}

/// Include WCET values in GCD computation for time step calculation.
fn collect_wcet_values(sm: &StateMachine) -> Vec<u64> {
    let mut values = Vec::new();
    for state in &sm.states {
        for action in state
            .entry_actions
            .iter()
            .chain(state.do_actions.iter())
            .chain(state.exit_actions.iter())
        {
            if let Some(wcet) = action.wcet_us {
                if wcet > 0 {
                    values.push(wcet);
                }
            }
        }
    }
    values
}

fn wcet_compute_time_step(timing: &TimingInfo, sm: &StateMachine) -> u64 {
    let mut time_values: Vec<u64> = Vec::new();
    if let Some(p) = timing.execution_period_ms {
        time_values.push(p * 1000);
    }
    if let Some(d) = timing.max_execution_time_us {
        time_values.push(d);
    }
    if let Some(j) = timing.max_jitter_us {
        time_values.push(j);
    }
    if let Some(db) = timing.debounce_period_ms {
        time_values.push(db * 1000);
    }
    time_values.extend(collect_wcet_values(sm));

    if time_values.is_empty() {
        return 1000;
    }
    let mut g = time_values[0];
    for &v in &time_values[1..] {
        g = gcd(g, v);
    }
    g.max(1)
}

fn wcet_write_phase_maps(out: &mut String, phase_ticks: &[(String, u64)]) {
    out.push_str("\n% WCET phase constants (in ticks)\n");
    for (sname, ticks) in phase_ticks {
        buf::append(
            out,
            format_args!("map PHASE_{sname}: Nat;\neqn PHASE_{sname} = {ticks};\n"),
        );
    }
    out.push('\n');
}

fn wcet_write_process_tail(
    out: &mut String,
    first_choice: &mut bool,
    proc_name: &str,
    part: &PartDef,
    info: &AttrInfo,
    timing: &TimingInfo,
    ticks: &TimedTickConstants,
) {
    let vars = build_timed_passthrough_vars(&info.state_vars);
    if is_periodic(timing) {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        if ticks.jitter_ticks > 0 {
            buf::append(out, format_args!(
                "{prefix}(phase == 0 && elapsed >= PERIOD_TICKS - JITTER_TICKS && elapsed <= PERIOD_TICKS + JITTER_TICKS) -> activate . {proc_name}(s, {vars}, 0, 0)\n"));
        } else {
            buf::append(out, format_args!(
                "{prefix}(phase == 0 && elapsed >= PERIOD_TICKS) -> activate . {proc_name}(s, {vars}, 0, 0)\n"));
        }
    }

    {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        buf::append(
            out,
            format_args!(
                "{prefix}(phase > 0) -> tick . {proc_name}(s, {vars}, elapsed + 1, Int2Nat(phase - 1))\n"
            ),
        );
    }

    {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        buf::append(out, format_args!(
            "{prefix}(phase == 0 && elapsed < MAX_ELAPSED) -> tick . {proc_name}(s, {vars}, elapsed + 1, 0)\n"));
    }

    if is_periodic(timing) {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        buf::append(out, format_args!(
            "{prefix}(phase > 0 && elapsed >= PERIOD_TICKS) -> deadline_miss . {proc_name}(s, {vars}, elapsed, 0)\n"));
    }

    let ungrouped = mcrl2_ungrouped_inputs(part, info);
    if is_debounce(timing) {
        wcet_write_debounce_env(out, first_choice, proc_name, part, info, &ungrouped);
    } else {
        wcet_write_plain_env(out, first_choice, proc_name, part, info, &ungrouped);
    }
}

fn wcet_write_debounce_env(
    out: &mut String,
    first_choice: &mut bool,
    proc_name: &str,
    part: &PartDef,
    info: &AttrInfo,
    ungrouped: &[&InputInfo],
) {
    for input in ungrouped {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sort = mcrl2_sort(&input.typ);
        let bound_var = format!("{}_v", input.name);
        let args = build_wcet_env_args(&info.state_vars, &input.name, &bound_var);
        buf::append(out, format_args!(
            "{prefix}(phase == 0 && elapsed >= DEBOUNCE_TICKS) -> sum {bound_var}: {sort} . env_{iname}({bound_var}) . {proc_name}({args})\n",
            iname = input.name));
    }
    for group in &part.input_groups {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sum_vars: Vec<String> = group
            .members
            .iter()
            .map(|m| {
                let sort = info
                    .inputs
                    .iter()
                    .find(|i| i.name == *m)
                    .map_or("Bool", |i| mcrl2_sort(&i.typ));
                format!("{m}_v: {sort}")
            })
            .collect();
        let action_args: Vec<String> = group.members.iter().map(|m| format!("{m}_v")).collect();
        let args = build_wcet_group_env_args(&info.state_vars, &group.members);
        buf::append(out, format_args!(
            "{prefix}(phase == 0 && elapsed >= DEBOUNCE_TICKS) -> sum {} . env_{}({}) . {proc_name}({args})\n",
            sum_vars.join(", "),
            group.name,
            action_args.join(", ")));
    }
}

fn wcet_write_plain_env(
    out: &mut String,
    first_choice: &mut bool,
    proc_name: &str,
    part: &PartDef,
    info: &AttrInfo,
    ungrouped: &[&InputInfo],
) {
    for input in ungrouped {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sort = mcrl2_sort(&input.typ);
        let bound_var = format!("{}_v", input.name);
        let args = build_wcet_env_args(&info.state_vars, &input.name, &bound_var);
        buf::append(
            out,
            format_args!(
                "{prefix}sum {bound_var}: {sort} . env_{iname}({bound_var}) . {proc_name}({args})\n",
                iname = input.name
            ),
        );
    }
    for group in &part.input_groups {
        let prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let sum_vars: Vec<String> = group
            .members
            .iter()
            .map(|m| {
                let sort = info
                    .inputs
                    .iter()
                    .find(|i| i.name == *m)
                    .map_or("Bool", |i| mcrl2_sort(&i.typ));
                format!("{m}_v: {sort}")
            })
            .collect();
        let action_args: Vec<String> = group.members.iter().map(|m| format!("{m}_v")).collect();
        let args = build_wcet_group_env_args(&info.state_vars, &group.members);
        buf::append(
            out,
            format_args!(
                "{prefix}sum {} . env_{}({}) . {proc_name}({args})\n",
                sum_vars.join(", "),
                group.name,
                action_args.join(", ")
            ),
        );
    }
}

/// Render the WCET-aware timed mCRL2 module with phase parameter.
fn render_wcet_timed_mcrl2_module(
    part: &PartDef,
    sm: &StateMachine,
    info: &AttrInfo,
    timing: &TimingInfo,
) -> String {
    let mut out = String::new();
    let name = &part.name;
    let time_step = wcet_compute_time_step(timing, sm);
    let period_ticks = timing
        .execution_period_ms
        .map_or(0, |p| p * 1000 / time_step);
    let jitter_ticks = timing.max_jitter_us.map_or(0, |j| j / time_step);
    let debounce_ticks = timing
        .debounce_period_ms
        .map_or(0, |d| d * 1000 / time_step);
    let max_elapsed = if is_periodic(timing) {
        period_ticks + jitter_ticks + 1
    } else {
        debounce_ticks + 1
    };
    let ticks = TimedTickConstants {
        time_step,
        period_ticks,
        jitter_ticks,
        debounce_ticks,
        max_elapsed,
    };
    let phase_ticks: Vec<(String, u64)> = sm
        .states
        .iter()
        .map(|s| (s.name.clone(), compute_state_phase_ticks(s, time_step)))
        .collect();

    buf::append(
        &mut out,
        format_args!("% Timed mCRL2 specification for {name} (WCET phase model)\n"),
    );
    out.push_str("% Generated from SysML model (discrete tick-based time with WCET phases)\n");
    buf::append(
        &mut out,
        format_args!("% Time step: {}us\n\n", ticks.time_step),
    );

    let state_names: Vec<&str> = sm.states.iter().map(|s| s.name.as_str()).collect();
    buf::append(
        &mut out,
        format_args!("sort State = struct {};\n\n", state_names.join(" | ")),
    );

    timed_write_timing_maps(&mut out, timing, &ticks);
    wcet_write_phase_maps(&mut out, &phase_ticks);
    mcrl2_write_constants(&mut out, part, info);

    out.push_str("act step: State # State;\n");
    out.push_str("    tick;\n");
    if is_periodic(timing) {
        out.push_str("    deadline_miss;\n");
        out.push_str("    activate;\n");
    }
    mcrl2_write_env_action_decls(&mut out, part, info);

    let mut params: Vec<String> = vec!["s: State".to_string()];
    for v in &info.state_vars {
        params.push(format!("{}: {}", v.name, mcrl2_sort(&v.typ)));
    }
    params.push("elapsed: Nat".to_string());
    params.push("phase: Nat".to_string());
    buf::append(
        &mut out,
        format_args!("proc {}({}) =\n", name, params.join(", ")),
    );

    let initial_state = &sm.states[0].name;
    let mut first_choice = true;
    Mcrl2StateChoices {
        out: &mut out,
        first_choice: &mut first_choice,
        proc_name: name,
        sm,
        info,
        guard_lead: "phase == 0 && ",
        tail: Mcrl2TransitionTail::Wcet,
        self_loop: Mcrl2SelfLoopKind::Wcet,
    }
    .write_all();
    wcet_write_process_tail(
        &mut out,
        &mut first_choice,
        name,
        part,
        info,
        timing,
        &ticks,
    );

    out.push_str("  ;\n\n");

    let initial_phase = format!("PHASE_{initial_state}");
    let init_args: Vec<String> = std::iter::once(initial_state.clone())
        .chain(info.state_vars.iter().map(|v| v.default.clone()))
        .chain(std::iter::once("0".to_string()))
        .chain(std::iter::once(initial_phase))
        .collect();
    buf::append(
        &mut out,
        format_args!("init {}({});\n", name, init_args.join(", ")),
    );

    out
}

/// Build recursive call arguments for a WCET self-loop (phase stays 0).
fn build_wcet_self_loop_args(
    _sm: &StateMachine,
    state_vars: &[VarInfo],
    _input_names: &BTreeSet<String>,
    current_state: &str,
    during: &[(String, String)],
    leaving: &[(String, String)],
) -> String {
    let mut args: Vec<String> = vec![current_state.to_string()];

    for v in state_vars {
        if let Some((_, expr)) = leaving.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else if let Some((_, expr)) = during.iter().find(|(var, _)| *var == v.name) {
            args.push(mcrl2_expr::sysml_expr_to_mcrl2(expr));
        } else {
            args.push(v.name.clone());
        }
    }

    args.push("elapsed".to_string());
    args.push("0".to_string());
    args.join(", ")
}

/// Build args for env input action in WCET model (passes elapsed, resets phase to 0).
fn build_wcet_env_args(state_vars: &[VarInfo], input_name: &str, bound_var: &str) -> String {
    let mut args: Vec<String> = vec!["s".to_string()];
    for v in state_vars {
        if v.name == input_name {
            args.push(bound_var.to_string());
        } else {
            args.push(v.name.clone());
        }
    }
    // Reset elapsed on env input for debounce actors, keep phase at 0
    args.push("0".to_string());
    args.push("0".to_string());
    args.join(", ")
}

/// Build args for grouped env input action in WCET model.
fn build_wcet_group_env_args(state_vars: &[VarInfo], members: &[String]) -> String {
    let mut args: Vec<String> = vec!["s".to_string()];
    for v in state_vars {
        if let Some(pos) = members.iter().position(|m| *m == v.name) {
            args.push(format!("{}_v", members[pos]));
        } else {
            args.push(v.name.clone());
        }
    }
    args.push("0".to_string());
    args.push("0".to_string());
    args.join(", ")
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
