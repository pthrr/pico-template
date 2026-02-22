//! SysML 2 text parser.
//!
//! Trusted boundary: parsing is complex and hard to verify.
//! The parser produces an AST that the verified codegen then translates.

use crate::ast::*;

/// Parse a SysML 2 source file into a Package AST.
pub fn parse_sysml(content: &str, filename: &str) -> Package {
    let pkg_name = extract_package_name(content)
        .unwrap_or_else(|| {
            std::path::Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

    let action_defs = parse_action_defs(content);
    let state_defs = parse_state_defs(content, &action_defs);
    let enums = parse_enums(content);
    let port_defs = parse_port_defs(content);
    let parts = parse_parts(content, &port_defs, &state_defs, &action_defs);

    Package {
        name: pkg_name,
        enums,
        port_defs,
        action_defs,
        state_defs,
        parts,
    }
}

fn extract_package_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("package ") {
            let rest = trimmed.strip_prefix("package ")?;
            let name = rest.trim().trim_end_matches('{').trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Parse package-level `action def name { body }` declarations.
/// Skips `entry action`, `do action`, `exit action` patterns (those are state action refs).
fn parse_action_defs(content: &str) -> Vec<ActionDef> {
    let mut defs = Vec::new();
    let mut i = 0;

    while i < content.len() {
        if let Some(pos) = content[i..].find("action def ") {
            let abs_pos = i + pos;

            // Make sure this is not inside a state def body (i.e., it's a package-level def)
            // We check that "action def" is not preceded by "entry ", "do ", or "exit "
            let prefix_start = if abs_pos >= 10 { abs_pos - 10 } else { 0 };
            let prefix = &content[prefix_start..abs_pos];
            if prefix.contains("entry ") || prefix.contains("do ") || prefix.contains("exit ") {
                i = abs_pos + 11;
                continue;
            }

            let name_start = abs_pos + 11; // len("action def ")
            if let Some(brace_offset) = content[name_start..].find('{') {
                let name = content[name_start..name_start + brace_offset].trim().to_string();
                let brace_start = name_start + brace_offset;

                if let Some((body, end_pos)) = extract_balanced(content, brace_start) {
                    defs.push(ActionDef {
                        name,
                        body,
                    });
                    i = end_pos;
                    continue;
                }
            }
            i = abs_pos + 11;
        } else {
            break;
        }
    }
    defs
}

/// Parse package-level `state def Name { ... }` declarations.
/// Resolves action references by looking up `action_defs`.
fn parse_state_defs(content: &str, action_defs: &[ActionDef]) -> Vec<State> {
    let mut states = Vec::new();
    let mut i = 0;

    while i < content.len() {
        if let Some(pos) = content[i..].find("state def ") {
            let abs_pos = i + pos;
            let name_start = abs_pos + 10; // len("state def ")

            if let Some(brace_offset) = content[name_start..].find('{') {
                let name = content[name_start..name_start + brace_offset].trim().to_string();
                let brace_start = name_start + brace_offset;

                if let Some((body, end_pos)) = extract_balanced(content, brace_start) {
                    let doc = extract_doc(&body);
                    let mut entry_actions = parse_action_refs(&body, "entry", action_defs);
                    let mut do_actions = parse_action_refs(&body, "do", action_defs);
                    let mut exit_actions = parse_action_refs(&body, "exit", action_defs);

                    // Also try inline actions (old format) as fallback
                    if entry_actions.is_empty() {
                        entry_actions = parse_actions(&body, "entry");
                    }
                    if do_actions.is_empty() {
                        do_actions = parse_actions(&body, "do");
                    }
                    if exit_actions.is_empty() {
                        exit_actions = parse_actions(&body, "exit");
                    }

                    states.push(State {
                        name,
                        doc,
                        entry_actions,
                        do_actions,
                        exit_actions,
                    });
                    i = end_pos;
                    continue;
                }
            }
            i = abs_pos + 10;
        } else {
            break;
        }
    }
    states
}

/// Parse `{kind} action : {name};` references inside a state def body.
/// Resolves the body from the corresponding action def.
fn parse_action_refs(body: &str, kind_str: &str, action_defs: &[ActionDef]) -> Vec<StateAction> {
    let mut actions = Vec::new();
    let pattern = format!("{kind_str} action : ");

    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&pattern) {
            let name = rest.trim_end_matches(';').trim().to_string();
            let kind = match kind_str {
                "entry" => ActionKind::Entry,
                "do" => ActionKind::Do,
                "exit" => ActionKind::Exit,
                _ => ActionKind::Do,
            };

            // Look up the action def body
            let action_body = action_defs.iter()
                .find(|ad| ad.name == name)
                .map(|ad| ad.body.clone());

            actions.push(StateAction {
                kind,
                name,
                body: action_body,
            });
        }
    }
    actions
}

fn parse_enums(content: &str) -> Vec<EnumDef> {
    let mut enums = Vec::new();
    let mut i = 0;

    while i < content.len() {
        if let Some(pos) = content[i..].find("enum def ") {
            let start = i + pos + 9; // skip "enum def "
            if let Some(brace) = content[start..].find('{') {
                let name = content[start..start + brace].trim().to_string();
                let body_start = start + brace + 1;
                if let Some(end) = content[body_start..].find('}') {
                    let body = &content[body_start..body_start + end];
                    let values: Vec<String> = body
                        .lines()
                        .map(|l| l.trim().trim_end_matches(';').trim().to_string())
                        .filter(|l| !l.is_empty() && !l.starts_with("//"))
                        .collect();
                    enums.push(EnumDef { name, values });
                    i = body_start + end + 1;
                    continue;
                }
            }
        }
        break;
    }
    enums
}

fn parse_port_defs(content: &str) -> Vec<Port> {
    let mut ports = Vec::new();
    let mut i = 0;

    while i < content.len() {
        if let Some(pos) = content[i..].find("port def ") {
            let start = i + pos + 9;
            if let Some(brace) = content[start..].find('{') {
                let name = content[start..start + brace].trim().to_string();
                let body_start = start + brace + 1;
                if let Some(end) = content[body_start..].find('}') {
                    let body = &content[body_start..body_start + end];
                    let signals = parse_signals(body);
                    let attributes = parse_attributes(body);
                    ports.push(Port {
                        name: name.clone(),
                        typ: name,
                        signals,
                        conjugated: false,
                        attributes,
                    });
                    i = body_start + end + 1;
                    continue;
                }
            }
        }
        break;
    }
    ports
}

fn parse_signals(body: &str) -> Vec<Signal> {
    let mut signals = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        // Match: (in|out) signal name : Type[N];
        let (direction, rest) = if let Some(r) = trimmed.strip_prefix("in signal ") {
            (SignalDirection::In, r)
        } else if let Some(r) = trimmed.strip_prefix("out signal ") {
            (SignalDirection::Out, r)
        } else {
            continue;
        };

        let rest = rest.trim_end_matches(';').trim();
        if let Some(colon) = rest.find(':') {
            let name = rest[..colon].trim().to_string();
            let type_part = rest[colon + 1..].trim();
            let (typ, array_size) = if let Some(bracket) = type_part.find('[') {
                let t = type_part[..bracket].trim().to_string();
                let size_str = type_part[bracket + 1..].trim_end_matches(']').trim();
                let size = size_str.parse().ok();
                (t, size)
            } else {
                (type_part.to_string(), None)
            };
            signals.push(Signal {
                name,
                typ,
                direction,
                array_size,
            });
        }
    }
    signals
}

fn parse_attributes(body: &str) -> Vec<Attribute> {
    let mut attrs = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("attribute ") {
            // Strip inline comments first, then semicolons
            let rest = if let Some(comment_pos) = rest.find("//") {
                rest[..comment_pos].trim()
            } else {
                rest
            };
            let rest = rest.trim_end_matches(';').trim();
            if let Some(colon) = rest.find(':') {
                let name_part = rest[..colon].trim();
                let type_and_default = rest[colon + 1..].trim();

                let (typ, default) = if let Some(eq) = type_and_default.find('=') {
                    let t = type_and_default[..eq].trim().to_string();
                    let d = type_and_default[eq + 1..].trim().to_string();
                    (t, Some(d))
                } else {
                    (type_and_default.to_string(), None)
                };

                // Handle comma-separated names
                for name in name_part.split(',') {
                    let name = name.trim().to_string();
                    if !name.is_empty() {
                        attrs.push(Attribute {
                            name,
                            typ: typ.clone(),
                            default: default.clone(),
                        });
                    }
                }
            }
        }
    }
    attrs
}

fn parse_parts(content: &str, port_defs: &[Port], state_defs: &[State], action_defs: &[ActionDef]) -> Vec<PartDef> {
    let mut parts = Vec::new();
    let mut search_start = 0;

    while search_start < content.len() {
        if let Some(pos) = content[search_start..].find("part def ") {
            let abs_pos = search_start + pos;
            let name_start = abs_pos + 9;

            if let Some(brace_offset) = content[name_start..].find('{') {
                let name = content[name_start..name_start + brace_offset].trim().to_string();
                let body_start = name_start + brace_offset;

                if let Some((body, end_pos)) = extract_balanced(content, body_start) {
                    let attributes = parse_attributes(&body);
                    let ports = parse_part_ports(&body, port_defs);
                    let state_machine = parse_state_machine(&body, state_defs, action_defs);
                    let input_groups = parse_input_groups(&body);
                    let part_instances = parse_part_instances(&body);
                    let connections = parse_connections(&body);

                    parts.push(PartDef {
                        name,
                        attributes,
                        ports,
                        state_machine,
                        input_groups,
                        part_instances,
                        connections,
                    });
                    search_start = end_pos;
                    continue;
                }
            }
        }
        break;
    }
    parts
}

fn parse_part_ports(body: &str, port_defs: &[Port]) -> Vec<Port> {
    let mut ports = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("port ") {
            let rest = rest.trim_end_matches(';').trim();
            if let Some(colon) = rest.find(':') {
                let name = rest[..colon].trim().to_string();
                let type_part = rest[colon + 1..].trim();
                let (conjugated, typ) = if let Some(t) = type_part.strip_prefix('~') {
                    (true, t.trim().to_string())
                } else {
                    (false, type_part.to_string())
                };

                if let Some(port_def) = port_defs.iter().find(|p| p.typ == typ) {
                    ports.push(Port {
                        name,
                        typ,
                        signals: port_def.signals.clone(),
                        conjugated,
                        attributes: port_def.attributes.clone(),
                    });
                } else {
                    // Port type not found in this file's port defs — create with
                    // empty signals (resolved later in cross-file composition)
                    ports.push(Port {
                        name,
                        typ,
                        signals: Vec::new(),
                        conjugated,
                        attributes: Vec::new(),
                    });
                }
            }
        }
    }
    ports
}

fn parse_state_machine(body: &str, state_defs: &[State], action_defs: &[ActionDef]) -> Option<StateMachine> {
    let sm_marker = "state machine";
    let sm_pos = body.find(sm_marker)?;
    let brace_pos = body[sm_pos..].find('{')?;
    let sm_body_start = sm_pos + brace_pos;

    let (sm_body, _) = extract_balanced(body, sm_body_start)?;

    let transitions = parse_transitions(&sm_body, action_defs);

    // Check if this is an exhibited state machine (uses `state : Name;` references)
    // vs inline state machine (uses `state name { ... }` blocks)
    let has_state_refs = sm_body.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("state : ") && trimmed.ends_with(';')
    });

    let states = if has_state_refs {
        parse_exhibited_states(&sm_body, state_defs, action_defs)
    } else {
        // Remove transition lines before parsing inline states
        let clean_lines: Vec<&str> = sm_body
            .lines()
            .filter(|l| !l.trim().starts_with("transition"))
            .collect();
        let clean_body = clean_lines.join("\n");
        parse_states(&clean_body)
    };

    if states.is_empty() {
        return None;
    }

    Some(StateMachine {
        states,
        transitions,
    })
}

/// Parse `state : Name;` references in an exhibited state machine body.
/// Looks up the state def by name and clones it.
fn parse_exhibited_states(sm_body: &str, state_defs: &[State], action_defs: &[ActionDef]) -> Vec<State> {
    let mut states = Vec::new();

    for line in sm_body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("state : ") {
            let name = rest.trim_end_matches(';').trim().to_string();

            // Look up in package-level state_defs
            if let Some(sd) = state_defs.iter().find(|s| s.name == name) {
                states.push(sd.clone());
            } else {
                // State referenced but not found in defs — create empty state
                // This also handles cases where the state def might use action refs
                // that weren't resolved yet
                states.push(State {
                    name,
                    doc: None,
                    entry_actions: Vec::new(),
                    do_actions: Vec::new(),
                    exit_actions: Vec::new(),
                });
            }
        }
    }

    // If action_defs available, ensure all states have resolved actions
    if !action_defs.is_empty() {
        for state in &mut states {
            resolve_state_actions_for(state, action_defs);
        }
    }

    states
}

/// For a state, resolve any StateActions that have body: None by looking up action_defs.
fn resolve_state_actions_for(state: &mut State, action_defs: &[ActionDef]) {
    for action in state.entry_actions.iter_mut()
        .chain(state.do_actions.iter_mut())
        .chain(state.exit_actions.iter_mut())
    {
        if action.body.is_none() {
            if let Some(ad) = action_defs.iter().find(|ad| ad.name == action.name) {
                action.body = Some(ad.body.clone());
            }
        }
    }
}

fn parse_transitions(body: &str, action_defs: &[ActionDef]) -> Vec<Transition> {
    let mut transitions = Vec::new();

    // Normalize multi-line transitions into single statements.
    // Collect text between "transition" and ";" into single strings.
    let mut statements = Vec::new();
    let mut current: Option<String> = None;

    for line in body.lines() {
        let trimmed = line.trim();

        // Skip doc comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with("doc ") || trimmed.starts_with("//") {
            continue;
        }

        if trimmed.starts_with("transition") {
            // Start a new transition statement
            if let Some(stmt) = current.take() {
                statements.push(stmt);
            }
            current = Some(trimmed.to_string());
        } else if let Some(ref mut stmt) = current {
            // Continuation of a multi-line transition
            stmt.push(' ');
            stmt.push_str(trimmed);
        }

        // Check if current statement is complete (ends with ;)
        if let Some(ref stmt) = current {
            if stmt.ends_with(';') {
                statements.push(stmt.clone());
                current = None;
            }
        }
    }
    if let Some(stmt) = current {
        statements.push(stmt);
    }

    for stmt in &statements {
        let rest = stmt
            .strip_prefix("transition")
            .unwrap_or("")
            .trim();

        // Skip optional label word before "first"
        // Patterns: "first X ...", "label_name first X ..."
        let rest = if rest.starts_with("first ") {
            rest.strip_prefix("first ").unwrap_or(rest).trim()
        } else if let Some(first_pos) = rest.find(" first ") {
            // "label first FROM ..." — skip label
            rest[first_pos + 7..].trim()
        } else {
            rest
        };

        // Handle `accept event` keyword between FROM and `then`/`if`
        // Also handle `do action : name` between guard and `then`:
        //   FROM accept EVENT do action : foo then TO
        //   FROM accept EVENT if COND do action : foo then TO
        //   FROM if COND then TO  (unchanged)
        //   FROM then TO  (unchanged)
        if let Some(then_pos) = rest.find(" then ") {
            let before_then = rest[..then_pos].trim();
            let to_state = rest[then_pos + 6..].trim().trim_end_matches(';').trim().to_string();

            // Extract transition actions: `do action : name` before `then`
            let (before_actions, trans_actions) = extract_transition_actions(before_then, action_defs);

            let (from_state, condition, is_accept) = parse_from_and_condition(before_actions);

            transitions.push(Transition {
                from_state,
                to_state,
                condition,
                is_accept,
                actions: trans_actions,
            });
        }
    }
    transitions
}

/// Extract `do action : name` clauses from before-then text.
/// Returns (remaining_text, actions).
fn extract_transition_actions<'a>(text: &'a str, action_defs: &[ActionDef]) -> (&'a str, Vec<StateAction>) {
    let mut actions = Vec::new();

    // Look for " do action : name" pattern
    if let Some(do_pos) = text.find(" do action : ") {
        let before = text[..do_pos].trim();
        let action_name = text[do_pos + 13..].trim().to_string();

        let action_body = action_defs.iter()
            .find(|ad| ad.name == action_name)
            .map(|ad| ad.body.clone());

        actions.push(StateAction {
            kind: ActionKind::Do,
            name: action_name,
            body: action_body,
        });

        return (before, actions);
    }

    (text, actions)
}

/// Parse the part before `then` to extract from_state, optional condition, and accept flag.
/// Handles:
///   "FROM" -> (FROM, None, false)
///   "FROM if COND" -> (FROM, Some(COND), false)
///   "FROM accept EVENT" -> (FROM, Some(EVENT), true)
///   "FROM accept EVENT if COND" -> (FROM, Some(EVENT and COND), true)
fn parse_from_and_condition(before_then: &str) -> (String, Option<String>, bool) {
    // Check for `accept` keyword
    if let Some(accept_pos) = before_then.find(" accept ") {
        let from = before_then[..accept_pos].trim().to_string();
        let after_accept = before_then[accept_pos + 8..].trim();

        // Check if there's also an `if` after accept
        if let Some(if_pos) = after_accept.find(" if ") {
            let event = after_accept[..if_pos].trim();
            let cond = after_accept[if_pos + 4..].trim();
            let combined = format!("{event} and {cond}");
            (from, Some(combined), true)
        } else {
            (from, Some(after_accept.to_string()), true)
        }
    } else if let Some(if_pos) = before_then.find(" if ") {
        let from = before_then[..if_pos].trim().to_string();
        let cond = before_then[if_pos + 4..].trim().to_string();
        (from, Some(cond), false)
    } else {
        (before_then.to_string(), None, false)
    }
}

fn parse_states(body: &str) -> Vec<State> {
    let mut states = Vec::new();
    let mut search_start = 0;

    while search_start < body.len() {
        // Find "state NAME {"
        if let Some(pos) = body[search_start..].find("state ") {
            let abs_pos = search_start + pos;
            let name_start = abs_pos + 6;

            // Skip "state machine" (already extracted)
            if body[name_start..].starts_with("machine") {
                search_start = name_start + 7;
                continue;
            }

            // Skip "state : Name;" (exhibited state references)
            if body[name_start..].starts_with(": ") {
                search_start = name_start + 2;
                continue;
            }

            if let Some(brace_offset) = body[name_start..].find('{') {
                let name = body[name_start..name_start + brace_offset].trim().to_string();
                let brace_start = name_start + brace_offset;

                if let Some((state_body, end_pos)) = extract_balanced(body, brace_start) {
                    let doc = extract_doc(&state_body);
                    let entry_actions = parse_actions(&state_body, "entry");
                    let do_actions = parse_actions(&state_body, "do");
                    let exit_actions = parse_actions(&state_body, "exit");

                    states.push(State {
                        name,
                        doc,
                        entry_actions,
                        do_actions,
                        exit_actions,
                    });
                    search_start = end_pos;
                    continue;
                }
            }
        }
        break;
    }
    states
}

fn parse_actions(body: &str, kind_str: &str) -> Vec<StateAction> {
    let mut actions = Vec::new();
    let pattern = format!("{} action ", kind_str);
    let mut search_start = 0;

    while search_start < body.len() {
        if let Some(pos) = body[search_start..].find(&pattern) {
            let abs_pos = search_start + pos;
            let name_start = abs_pos + pattern.len();

            // Check if this is an action reference (`: name;`) rather than inline action
            let after = body[name_start..].trim_start();
            if after.starts_with(": ") {
                // This is a reference, not an inline definition — skip
                search_start = name_start + 1;
                continue;
            }

            if let Some(brace_offset) = body[name_start..].find('{') {
                let name = body[name_start..name_start + brace_offset].trim().to_string();
                let brace_start = name_start + brace_offset;

                if let Some((action_body, end_pos)) = extract_balanced(body, brace_start) {
                    let kind = match kind_str {
                        "entry" => ActionKind::Entry,
                        "do" => ActionKind::Do,
                        "exit" => ActionKind::Exit,
                        _ => ActionKind::Do,
                    };
                    actions.push(StateAction {
                        kind,
                        name,
                        body: Some(action_body),
                    });
                    search_start = end_pos;
                    continue;
                }
            }
        }
        break;
    }
    actions
}

fn extract_doc(body: &str) -> Option<String> {
    if let Some(doc_start) = body.find("doc") {
        if let Some(open) = body[doc_start..].find("/*") {
            let text_start = doc_start + open + 2;
            if let Some(close) = body[text_start..].find("*/") {
                let raw = body[text_start..text_start + close].trim();
                let cleaned = raw.trim_matches('"').trim();
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }
    None
}

/// Parse `input_group name { var1, var2 }` declarations inside a part body.
fn parse_input_groups(body: &str) -> Vec<InputGroup> {
    let mut groups = Vec::new();
    let pattern = "input_group ";
    let mut search_start = 0;

    while search_start < body.len() {
        if let Some(pos) = body[search_start..].find(pattern) {
            let abs_pos = search_start + pos;
            let name_start = abs_pos + pattern.len();

            if let Some(brace_offset) = body[name_start..].find('{') {
                let name = body[name_start..name_start + brace_offset].trim().to_string();
                let brace_start = name_start + brace_offset;

                if let Some((group_body, end_pos)) = extract_balanced(body, brace_start) {
                    let members: Vec<String> = group_body
                        .split(',')
                        .map(|s| s.trim().trim_end_matches(';').trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    if !members.is_empty() {
                        groups.push(InputGroup { name, members });
                    }
                    search_start = end_pos;
                    continue;
                }
            }
            search_start = abs_pos + pattern.len();
        } else {
            break;
        }
    }
    groups
}

/// Parse `part name : Type;` lines inside a part def body.
/// Distinguishes from `part def`, `port`, and `state` lines.
fn parse_part_instances(body: &str) -> Vec<PartInstance> {
    let mut instances = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        // Must start with "part " but NOT "part def "
        if let Some(rest) = trimmed.strip_prefix("part ") {
            if rest.starts_with("def ") {
                continue;
            }
            let rest = rest.trim_end_matches(';').trim();
            if let Some(colon) = rest.find(':') {
                let name = rest[..colon].trim().to_string();
                let typ = rest[colon + 1..].trim().to_string();
                if !name.is_empty() && !typ.is_empty() {
                    instances.push(PartInstance { name, typ });
                }
            }
        }
    }
    instances
}

/// Parse `connection from.port to to.port;` or `connection from.port to to.port, CAPACITY;`
/// lines inside a part def body.
fn parse_connections(body: &str) -> Vec<Connection> {
    let mut connections = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("connection ") {
            let rest = rest.trim_end_matches(';').trim();
            // Split on " to "
            if let Some(to_pos) = rest.find(" to ") {
                let from_part_port = rest[..to_pos].trim();
                let to_and_capacity = rest[to_pos + 4..].trim();

                // Parse from: "part.port"
                let (from_part, from_port) = if let Some(dot) = from_part_port.find('.') {
                    (from_part_port[..dot].trim().to_string(),
                     from_part_port[dot + 1..].trim().to_string())
                } else {
                    continue;
                };

                // Parse to + optional capacity: "part.port" or "part.port, 4"
                let (to_part_port, capacity) = if let Some(comma) = to_and_capacity.find(',') {
                    let tp = to_and_capacity[..comma].trim();
                    let cap_str = to_and_capacity[comma + 1..].trim();
                    let cap = cap_str.parse::<usize>().unwrap_or(2);
                    (tp, cap)
                } else {
                    (to_and_capacity, 2)
                };

                let (to_part, to_port) = if let Some(dot) = to_part_port.find('.') {
                    (to_part_port[..dot].trim().to_string(),
                     to_part_port[dot + 1..].trim().to_string())
                } else {
                    continue;
                };

                connections.push(Connection {
                    from_part,
                    from_port,
                    to_part,
                    to_port,
                    capacity,
                });
            }
        }
    }
    connections
}

/// Extract content within balanced braces starting at `pos` (which points to '{').
fn extract_balanced(text: &str, pos: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if pos >= bytes.len() || bytes[pos] != b'{' {
        return None;
    }

    let mut level = 0;
    let mut i = pos;
    let mut start = pos + 1;

    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                if level == 0 {
                    start = i + 1;
                }
                level += 1;
            }
            b'}' => {
                level -= 1;
                if level == 0 {
                    return Some((text[start..i].to_string(), i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
