//! Composed mCRL2 specification generation for multi-actor systems.
//!
//! Given a system part def with part instances and connections, generates a
//! single mCRL2 module that composes all actor state machines with bounded
//! buffer processes for inter-actor communication.

use crate::ast::{Attribute, PartDef, Port, Signal, State, StateMachine, Transition};
use crate::buf;
use crate::mcrl2_expr;
use crate::mcrl2_render::TransitionAssigns;
use std::collections::{BTreeSet, HashMap};

struct PrefixedStepArgs<'a> {
    prefix: &'a str,
    vars: &'a [(String, String, String)],
    target_state: &'a str,
    assigns: TransitionAssigns<'a>,
    part_def: &'a PartDef,
}

struct ActorStateChoicesCtx<'a> {
    prefix: &'a str,
    cap: &'a str,
    proc_name: &'a str,
    sm: &'a StateMachine,
    part_def: &'a PartDef,
    actor: &'a ActorVarInfo,
}

struct ActorStateBranches<'a> {
    prefixed_state: String,
    do_assigns: &'a [(String, String)],
    exit_assigns: &'a [(String, String)],
    conditional: &'a [&'a Transition],
    unconditional: &'a [&'a Transition],
}

/// Channel metadata derived from a connection.
struct ChannelInfo {
    /// Source part instance name.
    from_part: String,
    /// Source port name.
    from_port: String,
    /// Destination part instance name.
    to_part: String,
    /// Destination port name.
    to_port: String,
    /// Port signals for the channel message.
    port_signals: Vec<Signal>,
    /// Channel capacity.
    capacity: usize,
}

/// Per-actor info for the composed spec.
struct ActorVarInfo {
    /// Instance name prefix.
    prefix: String,
    /// State names from the state machine.
    state_names: Vec<String>,
    /// Mutable variables (unprefixed name, type, default).
    vars: Vec<(String, String, String)>,
    /// Constants (unprefixed name, default).
    constants: Vec<(String, Option<String>)>,
}

/// Render a composed mCRL2 spec for a system part def.
/// Returns `(mcrl2_content, [(prop_name, mcf_content)])`.
pub fn render_composed_mcrl2(
    system: &PartDef,
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> (String, Vec<(String, String)>) {
    let channels = build_channels(system, parts, all_ports);
    let actors = build_actor_info(system, parts, all_ports);

    let mcrl2 = render_mcrl2_module(system, &actors, &channels, parts, all_ports);
    let props = render_mcf_properties(system, &actors);

    (mcrl2, props)
}

fn build_channels(
    system: &PartDef,
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> Vec<ChannelInfo> {
    let mut channels = Vec::new();

    for conn in &system.connections {
        let port_signals = find_port_signals(&conn.from_part, &conn.from_port, parts, all_ports);

        channels.push(ChannelInfo {
            from_part: conn.from_part.clone(),
            from_port: conn.from_port.clone(),
            to_part: conn.to_part.clone(),
            to_port: conn.to_port.clone(),
            port_signals,
            capacity: conn.capacity,
        });
    }

    channels
}

fn find_port_signals(
    instance: &str,
    port: &str,
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> Vec<Signal> {
    if let Some(part_def) = parts.get(instance) {
        if let Some(inst_port) = part_def.ports.iter().find(|p| p.name == port) {
            return inst_port.signals.clone();
        }
    }
    for port_def in all_ports {
        if port_def.name == port {
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
        let state_names: Vec<String> = sm.states.iter().map(|s| s.name.clone()).collect();

        // Classify attributes
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

        let mut input_names: BTreeSet<String> = BTreeSet::new();
        for t in &sm.transitions {
            if let Some(var) = &t.accept_var {
                input_names.insert(extract_accept_var_name(var.trim()));
            } else if t.is_accept {
                if let Some(cond) = &t.condition {
                    for word in cond.split(" and ") {
                        input_names.insert(extract_accept_var_name(word.trim()));
                    }
                }
            }
        }

        let mut vars = Vec::new();
        let mut constants = Vec::new();

        for attr in &part_def.attributes {
            if input_names.contains(&attr.name) || assigned.contains(&attr.name) {
                let default = mcrl2_default(&attr.typ, attr.default.as_deref());
                vars.push((attr.name.clone(), attr.typ.clone(), default));
            } else {
                constants.push((attr.name.clone(), attr.default.clone()));
            }
        }

        // Add port signal variables
        let resolved_ports: Vec<&Port> = part_def
            .ports
            .iter()
            .map(|p| {
                if p.signals.is_empty() {
                    all_ports
                        .iter()
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
                if vars.iter().any(|(n, _, _)| *n == sig.name) {
                    continue;
                }
                if let Some(pos) = constants.iter().position(|(n, _)| *n == sig.name) {
                    let (_, default_opt) = constants.remove(pos);
                    let default = mcrl2_default(&sig.typ, default_opt.as_deref());
                    vars.push((sig.name.clone(), sig.typ.clone(), default));
                } else {
                    let default = mcrl2_default(&sig.typ, None);
                    vars.push((sig.name.clone(), sig.typ.clone(), default));
                }
            }
        }

        actors.push(ActorVarInfo {
            prefix: prefix.clone(),
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

fn mcrl2_default(typ: &str, default: Option<&str>) -> String {
    if let Some(val) = default {
        return mcrl2_expr::sysml_expr_to_mcrl2(val);
    }
    match typ {
        "Boolean" => "false".to_string(),
        _ => "0".to_string(),
    }
}

fn mcrl2_sort(typ: &str) -> &'static str {
    match typ {
        "Boolean" => "Bool",
        "Real" => "Int",
        _ => "Nat",
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
    }
}

fn render_mcrl2_module(
    system: &PartDef,
    actors: &[ActorVarInfo],
    channels: &[ChannelInfo],
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) -> String {
    let mut out = String::new();
    let name = &system.name;

    buf::append(
        &mut out,
        format_args!("% Composed mCRL2 specification for {name}\n"),
    );
    out.push_str("% Generated from SysML model\n\n");
    for ch in channels {
        buf::append(
            &mut out,
            format_args!(
                "% {}.{} -> {}.{} (cap {})\n",
                ch.from_part, ch.from_port, ch.to_part, ch.to_port, ch.capacity
            ),
        );
    }
    if !channels.is_empty() {
        out.push('\n');
    }

    // Prefixed state sorts per actor
    for actor in actors {
        let cap = capitalize_first(&actor.prefix);
        let prefixed_names: Vec<String> = actor
            .state_names
            .iter()
            .map(|s| format!("{cap}{s}"))
            .collect();
        buf::append(
            &mut out,
            format_args!("sort {cap}State = struct {};\n", prefixed_names.join(" | ")),
        );
    }
    out.push('\n');

    // Message sorts from port signals
    let mut generated_msg_sorts: Vec<String> = Vec::new();
    for ch in channels {
        if !ch.port_signals.is_empty() {
            let msg_sort = format!("{}Msg", capitalize_first(&ch.from_part));
            if generated_msg_sorts.contains(&msg_sort) {
                continue;
            }
            let fields: Vec<String> = ch
                .port_signals
                .iter()
                .map(|s| format!("{}: {}", s.name, mcrl2_sort(&s.typ)))
                .collect();
            let ctor_name = format!("{}_msg", ch.from_part);
            buf::append(
                &mut out,
                format_args!(
                    "sort {msg_sort} = struct {ctor_name}({});\n",
                    fields.join(", ")
                ),
            );
            generated_msg_sorts.push(msg_sort.clone());
        }
    }
    if !channels.is_empty() {
        out.push('\n');
    }

    // Constants as map/eqn
    for actor in actors {
        let part_def = parts.get(&actor.prefix);
        for (cname, cdefault) in &actor.constants {
            let sort = part_def
                .and_then(|pd| pd.attributes.iter().find(|a| a.name == *cname))
                .map_or("Int", |a| mcrl2_sort(&a.typ));
            let val = match cdefault {
                Some(d) => mcrl2_expr::sysml_expr_to_mcrl2(d),
                None => "0".to_string(),
            };
            let prefixed = format!("{}_{cname}", actor.prefix);
            buf::append(&mut out, format_args!("map {prefixed}: {sort};\n"));
            buf::append(&mut out, format_args!("eqn {prefixed} = {val};\n"));
        }
    }
    if actors.iter().any(|a| !a.constants.is_empty()) {
        out.push('\n');
    }

    compose_write_actions_and_processes(&mut out, system, actors, channels, parts, all_ports);
    compose_write_init_block(&mut out, actors, channels);
    out
}

fn compose_write_actions_and_processes(
    out: &mut String,
    system: &PartDef,
    actors: &[ActorVarInfo],
    channels: &[ChannelInfo],
    parts: &HashMap<String, &PartDef>,
    all_ports: &[&Port],
) {
    for actor in actors {
        let cap = capitalize_first(&actor.prefix);
        buf::append(
            out,
            format_args!("act {}_step: {cap}State # {cap}State;\n", actor.prefix),
        );
    }
    for ch in channels {
        let msg_sort = if ch.port_signals.is_empty() {
            "Bool".to_string()
        } else {
            format!("{}Msg", capitalize_first(&ch.from_part))
        };
        buf::append(
            out,
            format_args!("    send_{}_to_{}: {msg_sort};\n", ch.from_part, ch.to_part),
        );
        buf::append(
            out,
            format_args!("    recv_{}_to_{}: {msg_sort};\n", ch.from_part, ch.to_part),
        );
        buf::append(
            out,
            format_args!("    comm_{}_to_{}: {msg_sort};\n", ch.from_part, ch.to_part),
        );
    }
    out.push('\n');

    for inst in &system.part_instances {
        let Some(part_def) = parts.get(&inst.name) else {
            continue;
        };
        let Some(sm) = &part_def.state_machine else {
            continue;
        };
        let actor = actors
            .iter()
            .find(|a| a.prefix == inst.name)
            .expect("actor info");
        render_actor_process(out, &inst.name, sm, part_def, actor, channels, all_ports);
    }
    for ch in channels {
        render_buffer_process(out, ch);
    }
}

fn compose_write_init_block(out: &mut String, actors: &[ActorVarInfo], channels: &[ChannelInfo]) {
    out.push_str("init\n");

    let mut allow_actions: Vec<String> = Vec::new();
    for actor in actors {
        allow_actions.push(format!("{}_step", actor.prefix));
    }
    for ch in channels {
        allow_actions.push(format!("comm_{}_to_{}", ch.from_part, ch.to_part));
    }

    let mut comm_mappings: Vec<String> = Vec::new();
    for ch in channels {
        comm_mappings.push(format!(
            "send_{f}_to_{t} | recv_{f}_to_{t} -> comm_{f}_to_{t}",
            f = ch.from_part,
            t = ch.to_part
        ));
    }

    let mut proc_inits: Vec<String> = Vec::new();
    for actor in actors {
        let cap = capitalize_first(&actor.prefix);
        let initial_state = format!("{}{}", cap, actor.state_names[0]);
        let init_args: Vec<String> = std::iter::once(initial_state)
            .chain(actor.vars.iter().map(|(_, _, d)| d.clone()))
            .collect();
        proc_inits.push(format!(
            "{}({})",
            capitalize_first(&actor.prefix),
            init_args.join(", ")
        ));
    }
    for ch in channels {
        proc_inits.push(format!("Buffer_{}_to_{}([])", ch.from_part, ch.to_part));
    }

    if comm_mappings.is_empty() {
        buf::append(
            out,
            format_args!("  allow({{ {} }},\n", allow_actions.join(", ")),
        );
        buf::append(out, format_args!("    {}\n", proc_inits.join(" || ")));
        out.push_str("  );\n");
    } else {
        buf::append(
            out,
            format_args!("  allow({{ {} }},\n", allow_actions.join(", ")),
        );
        buf::append(
            out,
            format_args!("  comm({{ {} }},\n", comm_mappings.join(", ")),
        );
        buf::append(out, format_args!("    {}\n", proc_inits.join(" || ")));
        out.push_str("  ));\n");
    }
}

/// Render a single actor process with send/receive actions for channels.
fn render_actor_process(
    out: &mut String,
    prefix: &str,
    sm: &StateMachine,
    part_def: &PartDef,
    actor: &ActorVarInfo,
    channels: &[ChannelInfo],
    _all_ports: &[&Port],
) {
    let cap = capitalize_first(prefix);
    let proc_name = cap.clone();

    // Build parameter list
    let mut params: Vec<String> = vec![format!("s: {cap}State")];
    for (vname, vtyp, _) in &actor.vars {
        params.push(format!("{}_{}: {}", prefix, vname, mcrl2_sort(vtyp)));
    }

    buf::append(
        out,
        format_args!("proc {proc_name}({}) =\n", params.join(", ")),
    );

    let mut first_choice = true;
    let ctx = ActorStateChoicesCtx {
        prefix,
        cap: &cap,
        proc_name: &proc_name,
        sm,
        part_def,
        actor,
    };
    render_actor_state_choices(out, &mut first_choice, &ctx);
    render_actor_channel_actions(out, &mut first_choice, prefix, &proc_name, actor, channels);

    out.push_str("  ;\n\n");
}

fn render_actor_state_choices(
    out: &mut String,
    first_choice: &mut bool,
    ctx: &ActorStateChoicesCtx<'_>,
) {
    for state in &ctx.sm.states {
        render_one_actor_state_choice(out, first_choice, ctx, state);
    }
}

fn render_one_actor_state_choice(
    out: &mut String,
    first_choice: &mut bool,
    ctx: &ActorStateChoicesCtx<'_>,
    state: &State,
) {
    let transitions: Vec<&Transition> = ctx
        .sm
        .transitions
        .iter()
        .filter(|t| t.from_state == state.name)
        .collect();
    if transitions.is_empty() {
        return;
    }

    let prefixed_state = format!("{}{}", ctx.cap, state.name);
    let do_assigns = collect_do_assignments(state);
    let exit_assigns = collect_exit_assignments(state);
    let conditional: Vec<&Transition> = transitions
        .iter()
        .filter(|t| t.condition.is_some())
        .copied()
        .collect();
    let unconditional: Vec<&Transition> = transitions
        .iter()
        .filter(|t| t.condition.is_none())
        .copied()
        .collect();

    let branch = ActorStateBranches {
        prefixed_state,
        do_assigns: &do_assigns,
        exit_assigns: &exit_assigns,
        conditional: &conditional,
        unconditional: &unconditional,
    };
    compose_write_conditional(out, first_choice, ctx, &branch);
    compose_write_unconditional(out, first_choice, ctx, &branch);
    compose_write_self_loop(out, first_choice, ctx, &branch);
}

fn compose_prefixed_negated_guards(
    conditional: &[&Transition],
    prefix: &str,
    part_def: &PartDef,
) -> Vec<String> {
    conditional
        .iter()
        .map(|ct| {
            let cond = ct.condition.as_ref().expect("conditional has condition");
            let mcrl2_cond = prefix_condition(
                &mcrl2_expr::sysml_condition_to_mcrl2(cond),
                prefix,
                part_def,
            );
            format!("!({mcrl2_cond})")
        })
        .collect()
}

fn compose_write_conditional(
    out: &mut String,
    first_choice: &mut bool,
    ctx: &ActorStateChoicesCtx<'_>,
    branch: &ActorStateBranches<'_>,
) {
    for t in branch.conditional {
        let cond = t.condition.as_ref().expect("conditional has condition");
        let mcrl2_cond = prefix_condition(
            &mcrl2_expr::sysml_condition_to_mcrl2(cond),
            ctx.prefix,
            ctx.part_def,
        );
        let choice_prefix = if *first_choice { "  " } else { "  + " };
        *first_choice = false;
        let target = format!("{}{}", ctx.cap, t.to_state);
        let args = prefixed_step_args(PrefixedStepArgs {
            prefix: ctx.prefix,
            vars: &ctx.actor.vars,
            target_state: &target,
            assigns: TransitionAssigns {
                during: branch.do_assigns,
                leaving: branch.exit_assigns,
                stepping: &collect_transition_action_assignments(t),
                entering: &collect_entry_assignments_for_target(ctx.sm, &t.to_state),
            },
            part_def: ctx.part_def,
        });
        buf::append(out, format_args!(
            "{choice_prefix}(s == {prefixed_state} && {mcrl2_cond}) -> {prefix}_step({prefixed_state}, {target}) . {proc_name}({args})\n",
            prefixed_state = branch.prefixed_state,
            prefix = ctx.prefix,
            proc_name = ctx.proc_name
        ));
    }
}

fn compose_write_unconditional(
    out: &mut String,
    first_choice: &mut bool,
    ctx: &ActorStateChoicesCtx<'_>,
    branch: &ActorStateBranches<'_>,
) {
    if branch.unconditional.len() != 1 {
        return;
    }
    let t = branch.unconditional[0];
    let target = format!("{}{}", ctx.cap, t.to_state);
    let choice_prefix = if *first_choice { "  " } else { "  + " };
    *first_choice = false;
    let args = prefixed_step_args(PrefixedStepArgs {
        prefix: ctx.prefix,
        vars: &ctx.actor.vars,
        target_state: &target,
        assigns: TransitionAssigns {
            during: branch.do_assigns,
            leaving: branch.exit_assigns,
            stepping: &collect_transition_action_assignments(t),
            entering: &collect_entry_assignments_for_target(ctx.sm, &t.to_state),
        },
        part_def: ctx.part_def,
    });

    if branch.conditional.is_empty() {
        buf::append(out, format_args!(
            "{choice_prefix}(s == {prefixed_state}) -> {prefix}_step({prefixed_state}, {target}) . {proc_name}({args})\n",
            prefixed_state = branch.prefixed_state,
            prefix = ctx.prefix,
            proc_name = ctx.proc_name
        ));
        return;
    }

    let neg =
        compose_prefixed_negated_guards(branch.conditional, ctx.prefix, ctx.part_def).join(" && ");
    buf::append(out, format_args!(
        "{choice_prefix}(s == {prefixed_state} && {neg}) -> {prefix}_step({prefixed_state}, {target}) . {proc_name}({args})\n",
        prefixed_state = branch.prefixed_state,
        prefix = ctx.prefix,
        proc_name = ctx.proc_name
    ));
}

fn compose_write_self_loop(
    out: &mut String,
    first_choice: &mut bool,
    ctx: &ActorStateChoicesCtx<'_>,
    branch: &ActorStateBranches<'_>,
) {
    if !branch.unconditional.is_empty() || branch.conditional.is_empty() {
        return;
    }
    let neg =
        compose_prefixed_negated_guards(branch.conditional, ctx.prefix, ctx.part_def).join(" && ");
    let choice_prefix = if *first_choice { "  " } else { "  + " };
    *first_choice = false;
    let args = build_prefixed_self_loop_args(
        ctx.prefix,
        &ctx.actor.vars,
        &branch.prefixed_state,
        branch.do_assigns,
        branch.exit_assigns,
        ctx.part_def,
    );
    buf::append(out, format_args!(
        "{choice_prefix}(s == {prefixed_state} && {neg}) -> {prefix}_step({prefixed_state}, {prefixed_state}) . {proc_name}({args})\n",
        prefixed_state = branch.prefixed_state,
        prefix = ctx.prefix,
        proc_name = ctx.proc_name
    ));
}

fn render_actor_channel_actions(
    out: &mut String,
    first_choice: &mut bool,
    prefix: &str,
    proc_name: &str,
    actor: &ActorVarInfo,
    channels: &[ChannelInfo],
) {
    for ch in channels {
        if ch.from_part == prefix {
            let choice_prefix = if *first_choice { "  " } else { "  + " };
            *first_choice = false;

            if ch.port_signals.is_empty() {
                buf::append(
                    out,
                    format_args!(
                        "{choice_prefix}send_{f}_to_{t}(true) . {proc_name}(s, {passthrough})\n",
                        f = ch.from_part,
                        t = ch.to_part,
                        passthrough = build_passthrough_args(prefix, &actor.vars)
                    ),
                );
            } else {
                let msg_fields: Vec<String> = ch
                    .port_signals
                    .iter()
                    .map(|s| format!("{prefix}_{}", s.name))
                    .collect();
                let ctor_name = format!("{}_msg", ch.from_part);
                buf::append(out, format_args!("{choice_prefix}send_{f}_to_{t}({ctor_name}({fields})) . {proc_name}(s, {passthrough})\n",
                    f = ch.from_part, t = ch.to_part,
                    fields = msg_fields.join(", "),
                    passthrough = build_passthrough_args(prefix, &actor.vars)));
            }
        }
    }

    // Receive actions: for channels where this actor is the destination
    for ch in channels {
        if ch.to_part == prefix {
            let choice_prefix = if *first_choice { "  " } else { "  + " };
            *first_choice = false;

            if ch.port_signals.is_empty() {
                buf::append(out, format_args!("{choice_prefix}sum b: Bool . recv_{f}_to_{t}(b) . {proc_name}(s, {passthrough})\n",
                    f = ch.from_part, t = ch.to_part,
                    passthrough = build_passthrough_args(prefix, &actor.vars)));
            } else {
                let msg_sort = format!("{}Msg", capitalize_first(&ch.from_part));
                let bound_var = format!("m_{f}_to_{t}", f = ch.from_part, t = ch.to_part);
                // sum m: MsgSort . recv(m) . P(s, ..., m.field1, m.field2, ...)
                let recv_args = build_recv_args(prefix, &actor.vars, &ch.port_signals, &bound_var);
                buf::append(out, format_args!("{choice_prefix}sum {bound_var}: {msg_sort} . recv_{f}_to_{t}({bound_var}) . {proc_name}({recv_args})\n",
                    f = ch.from_part, t = ch.to_part));
            }
        }
    }
}

/// Render a buffer process for a channel.
fn render_buffer_process(out: &mut String, ch: &ChannelInfo) {
    let buf_name = format!("Buffer_{}_to_{}", ch.from_part, ch.to_part);
    let msg_sort = if ch.port_signals.is_empty() {
        "Bool".to_string()
    } else {
        format!("{}Msg", capitalize_first(&ch.from_part))
    };

    buf::append(
        out,
        format_args!("proc {buf_name}(q: List({msg_sort})) =\n"),
    );
    buf::append(
        out,
        format_args!(
            "  (#q < {cap}) -> sum m: {msg_sort} . recv_{f}_to_{t}(m) . {buf_name}(m |> q)\n",
            cap = ch.capacity,
            f = ch.from_part,
            t = ch.to_part
        ),
    );
    buf::append(
        out,
        format_args!(
            "  + (#q > 0) -> send_{f}_to_{t}(rhead(q)) . {buf_name}(rtail(q))\n",
            f = ch.from_part,
            t = ch.to_part
        ),
    );
    out.push_str("  ;\n\n");
}

fn prefixed_step_args(params: PrefixedStepArgs<'_>) -> String {
    let PrefixedStepArgs {
        prefix,
        vars,
        target_state,
        assigns,
        part_def,
    } = params;
    let TransitionAssigns {
        during,
        leaving,
        stepping,
        entering,
    } = assigns;

    let mut args: Vec<String> = vec![target_state.to_string()];

    for (vname, _, _) in vars {
        let prefixed = format!("{prefix}_{vname}");
        if let Some((_, expr)) = entering.iter().find(|(var, _)| *var == *vname) {
            args.push(prefix_expr(
                &mcrl2_expr::sysml_expr_to_mcrl2(expr),
                prefix,
                part_def,
            ));
        } else if let Some((_, expr)) = stepping.iter().find(|(var, _)| *var == *vname) {
            args.push(prefix_expr(
                &mcrl2_expr::sysml_expr_to_mcrl2(expr),
                prefix,
                part_def,
            ));
        } else if let Some((_, expr)) = leaving.iter().find(|(var, _)| *var == *vname) {
            args.push(prefix_expr(
                &mcrl2_expr::sysml_expr_to_mcrl2(expr),
                prefix,
                part_def,
            ));
        } else if let Some((_, expr)) = during.iter().find(|(var, _)| *var == *vname) {
            args.push(prefix_expr(
                &mcrl2_expr::sysml_expr_to_mcrl2(expr),
                prefix,
                part_def,
            ));
        } else {
            args.push(prefixed);
        }
    }

    args.join(", ")
}

fn build_prefixed_self_loop_args(
    prefix: &str,
    vars: &[(String, String, String)],
    current_state: &str,
    during: &[(String, String)],
    leaving: &[(String, String)],
    part_def: &PartDef,
) -> String {
    let mut args: Vec<String> = vec![current_state.to_string()];

    for (vname, _, _) in vars {
        let prefixed = format!("{prefix}_{vname}");
        if let Some((_, expr)) = leaving.iter().find(|(var, _)| *var == *vname) {
            args.push(prefix_expr(
                &mcrl2_expr::sysml_expr_to_mcrl2(expr),
                prefix,
                part_def,
            ));
        } else if let Some((_, expr)) = during.iter().find(|(var, _)| *var == *vname) {
            args.push(prefix_expr(
                &mcrl2_expr::sysml_expr_to_mcrl2(expr),
                prefix,
                part_def,
            ));
        } else {
            args.push(prefixed);
        }
    }

    args.join(", ")
}

fn build_passthrough_args(prefix: &str, vars: &[(String, String, String)]) -> String {
    vars.iter()
        .map(|(vname, _, _)| format!("{prefix}_{vname}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_recv_args(
    prefix: &str,
    vars: &[(String, String, String)],
    signals: &[Signal],
    bound_var: &str,
) -> String {
    let mut args: Vec<String> = vec!["s".to_string()];

    for (vname, _, _) in vars {
        if let Some(sig) = signals.iter().find(|s| s.name == *vname) {
            args.push(format!("{}({bound_var})", sig.name));
        } else {
            args.push(format!("{prefix}_{vname}"));
        }
    }

    args.join(", ")
}

/// Prefix bare variable references in an mCRL2 expression with the actor prefix.
fn prefix_expr(expr: &str, prefix: &str, part_def: &PartDef) -> String {
    let mut result = expr.to_string();
    let mut attrs: Vec<&Attribute> = part_def.attributes.iter().collect();
    attrs.sort_by(|a, b| b.name.len().cmp(&a.name.len()));

    for attr in attrs {
        let pattern = &attr.name;
        let replacement = format!("{prefix}_{}", attr.name);
        result = word_replace(&result, pattern, &replacement);
    }
    result
}

/// Prefix condition expression with actor prefix.
fn prefix_condition(expr: &str, prefix: &str, part_def: &PartDef) -> String {
    prefix_expr(expr, prefix, part_def)
}

/// Replace whole words only.
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

fn render_mcf_properties(system: &PartDef, actors: &[ActorVarInfo]) -> Vec<(String, String)> {
    let mut props = Vec::new();

    // Deadlock freedom
    let deadlock_name = format!("{}_deadlock_freedom", system.name);
    let deadlock_mcf = "[true*]<true>true\n".to_string();
    props.push((deadlock_name, deadlock_mcf));

    // Liveness: all actors eventually return to initial state
    // Uses step-only quantification to ignore env/comm action stutter.
    let liveness_name = format!("{}_liveness", system.name);
    let mut liveness_parts: Vec<String> = Vec::new();
    for actor in actors {
        let cap = capitalize_first(&actor.prefix);
        let initial = format!("{}{}", cap, actor.state_names[0]);
        let state_sort = format!("{cap}State");
        let step_action = format!("{}_step", actor.prefix);
        let source_states: Vec<String> = actor
            .state_names
            .iter()
            .map(|s| format!("{cap}{s}"))
            .collect();
        let step_disjuncts: Vec<String> = source_states
            .iter()
            .map(|s| format!("<{step_action}({s}, {initial})>X"))
            .collect();
        if !step_disjuncts.is_empty() {
            liveness_parts.push(format!(
                "nu X. mu Y. ({} || ((exists s1,s2: {state_sort} . <{step_action}(s1, s2)>Y) && (forall s1,s2: {state_sort} . [{step_action}(s1, s2)]Y)))",
                step_disjuncts.join(" || ")));
        }
    }

    let liveness_mcf = if liveness_parts.is_empty() {
        "true\n".to_string()
    } else if liveness_parts.len() == 1 {
        format!("{}\n", liveness_parts[0])
    } else {
        format!("({})\n", liveness_parts.join(") && ("))
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

/// Collect transition-action assignments.
fn collect_transition_action_assignments(t: &Transition) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for action in &t.actions {
        if let Some(body) = &action.body {
            result.extend(mcrl2_expr::parse_assignments(body));
        }
    }
    result
}
