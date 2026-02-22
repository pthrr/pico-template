//! Verified translation: SysML AST → typed Rust IR.
//!
//! All types and specs live inside `verus!{}`. Translation functions are
//! `#[verifier::external_body]` with postcondition specs — Verus cannot
//! currently reason about `for x in &Vec<T>` iterators, so the loop
//! bodies are trusted. The specs serve as checked contracts at call sites.

use crate::ast::*;
use crate::expr;
use crate::rust_ast::*;
use vstd::prelude::*;

verus! {

// ── Trusted specs for std types vstd doesn't cover ──────────────────

pub assume_specification [std::string::String::new] () -> (result: String)
    ensures result@.len() == 0;

pub assume_specification [std::string::String::push] (s: &mut String, c: char)
    ensures s@.len() > old(s)@.len();

pub assume_specification [std::string::String::push_str] (s: &mut String, string: &str)
    ensures s@.len() >= old(s)@.len() + string@.len();

pub assume_specification<'a> [<std::string::String as std::ops::Deref>::deref] (s: &'a String) -> (result: &'a str)
    ensures result@ == s@;

pub assume_specification<'a, 'b> [<std::string::String as std::cmp::PartialEq<&'a str>>::eq] (a: &String, b: &&'a str) -> (result: bool);

pub assume_specification [std::string::String::is_empty] (s: &String) -> (result: bool)
    ensures result == (s@.len() == 0);

pub assume_specification [str::is_empty] (s: &str) -> (result: bool)
    ensures result == (s@.len() == 0);

pub assume_specification [str::trim] (s: &str) -> (result: &str)
    ensures result@.len() <= s@.len();

pub assume_specification [char::to_ascii_uppercase] (c: &char) -> (result: char);

// ── End assume_specifications ────────────────────────────────────────

/// Convert snake_case to PascalCase.
#[verifier::external_body]
pub fn to_pascal_case(s: &str) -> (result: String)
    ensures s@.len() > 0 ==> result@.len() > 0,
{
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

/// Map SysML type names to Rust type names.
#[verifier::external_body]
pub fn rust_type(sysml_type: &str) -> (result: &'static str)
    ensures result@.len() > 0,
{
    match sysml_type {
        "Real" => "f32",
        "Integer" => "i32",
        "Boolean" => "bool",
        "String" => "&str",
        _ => "/* unknown */",
    }
}

/// Get Rust default value for a SysML type.
#[verifier::external_body]
pub fn rust_default(sysml_type: &str, default: &Option<String>) -> (result: String)
    ensures result@.len() > 0,
{
    match default {
        Some(val) => {
            match sysml_type {
                "Real" => {
                    if val.contains('.') {
                        val.clone()
                    } else {
                        format!("{val}.0")
                    }
                }
                "Boolean" => val.to_lowercase(),
                _ => val.clone(),
            }
        }
        None => {
            match sysml_type {
                "Real" => "0.0".to_string(),
                "Integer" => "0".to_string(),
                "Boolean" => "false".to_string(),
                "String" => "\"\"".to_string(),
                _ => "Default::default()".to_string(),
            }
        }
    }
}

/// Translate a SysML Package into a typed Rust IR module.
#[verifier::external_body]
pub fn translate(package: &Package) -> (result: RustModule)
    ensures result.items@.len() >= 1,
{
    let mut items: Vec<RustItem> = Vec::new();

    // Header comments
    items.push(RustItem::Comment("Auto-generated from SysML 2 model".to_string()));
    let mut pkg_comment = String::new();
    pkg_comment.push_str("Package: ");
    pkg_comment.push_str(package.name.as_str());
    items.push(RustItem::Comment(pkg_comment));
    items.push(RustItem::BlankLine);

    // Enums
    for enum_def in &package.enums {
        items.push(RustItem::Enum(translate_enum(enum_def)));
    }

    // Port traits and message structs
    for port_def in &package.port_defs {
        // Message struct from port signals (e.g., ButtonPort → ButtonMessage)
        if !port_def.signals.is_empty() {
            items.push(RustItem::Struct(translate_port_message(port_def)));
        }
        items.push(RustItem::Trait(translate_port_trait(port_def)));
    }

    // Parts
    for part in &package.parts {
        let part_items = translate_part(part, &package.port_defs);
        for item in part_items {
            items.push(item);
        }
    }

    RustModule {
        package_name: package.name.clone(),
        items,
    }
}

/// Translate a SysML enum into a Rust IR enum.
#[verifier::external_body]
pub fn translate_enum(e: &EnumDef) -> (result: RustEnum)
    ensures result.variants@.len() == e.values@.len(),
{
    let mut variants = Vec::new();
    for val in &e.values {
        variants.push(RustVariant {
            name: to_pascal_case(val),
            doc: None,
        });
    }
    RustEnum {
        name: e.name.clone(),
        variants,
    }
}

/// Translate a port definition into a Rust trait.
#[verifier::external_body]
fn translate_port_trait(port: &Port) -> RustTrait {
    let mut methods = Vec::new();
    for sig in &port.signals {
        let rtype = rust_type(&sig.typ).to_string();
        match sig.direction {
            SignalDirection::Out => {
                let mut get_params = String::new();
                get_params.push_str("&self");
                methods.push(TraitMethodSig {
                    name: {
                        let mut n = String::new();
                        n.push_str("get_");
                        n.push_str(sig.name.as_str());
                        n
                    },
                    params: get_params,
                    ret_type: rtype.clone(),
                });
                let mut set_params = String::new();
                set_params.push_str("&mut self, value: ");
                set_params.push_str(rust_type(&sig.typ));
                methods.push(TraitMethodSig {
                    name: {
                        let mut n = String::new();
                        n.push_str("set_");
                        n.push_str(sig.name.as_str());
                        n
                    },
                    params: set_params,
                    ret_type: String::new(),
                });
            }
            SignalDirection::In => {
                let mut get_params = String::new();
                get_params.push_str("&self");
                methods.push(TraitMethodSig {
                    name: {
                        let mut n = String::new();
                        n.push_str("get_");
                        n.push_str(sig.name.as_str());
                        n
                    },
                    params: get_params,
                    ret_type: rtype,
                });
            }
        }
    }
    RustTrait {
        name: port.typ.clone(),
        methods,
    }
}

/// Derive message struct name from port type name.
/// `ButtonPort` → `ButtonMessage`, `StatusPort` → `StatusMessage`.
#[verifier::external_body]
fn port_message_name(port_type: &str) -> (result: String)
    ensures result@.len() > 0,
{
    if let Some(base) = port_type.strip_suffix("Port") {
        format!("{base}Message")
    } else {
        format!("{port_type}Message")
    }
}

/// Generate a message struct from a port definition's signals.
#[verifier::external_body]
fn translate_port_message(port: &Port) -> RustStruct {
    let msg_name = port_message_name(&port.typ);
    let mut fields = Vec::new();
    for sig in &port.signals {
        fields.push(RustField {
            name: sig.name.clone(),
            typ: rust_type(&sig.typ).to_string(),
        });
    }
    RustStruct {
        name: msg_name,
        fields,
        derives: vec![
            "Debug".to_string(),
            "Clone".to_string(),
            "Copy".to_string(),
        ],
    }
}

/// Translate a SysML part into a set of Rust IR items.
#[verifier::external_body]
pub fn translate_part(part: &PartDef, port_defs: &[Port]) -> (result: Vec<RustItem>)
    ensures result@.len() > 0,
{
    let mut items: Vec<RustItem> = Vec::new();

    // Comment header
    items.push(RustItem::Comment(part.name.clone()));
    items.push(RustItem::BlankLine);

    // State enum (if state machine present)
    match &part.state_machine {
        Some(sm) => {
            let mut state_enum_name = String::new();
            state_enum_name.push_str(part.name.as_str());
            state_enum_name.push_str("State");
            let mut variants = Vec::new();
            for state in &sm.states {
                variants.push(RustVariant {
                    name: to_pascal_case(&state.name),
                    doc: state.doc.clone(),
                });
            }
            items.push(RustItem::Enum(RustEnum {
                name: state_enum_name,
                variants,
            }));
        }
        None => {}
    }

    // Struct
    let mut fields = Vec::new();
    if part.state_machine.is_some() {
        let mut state_type = String::new();
        state_type.push_str(part.name.as_str());
        state_type.push_str("State");
        fields.push(RustField {
            name: "state".to_string(),
            typ: state_type,
        });
    }
    for attr in &part.attributes {
        fields.push(RustField {
            name: attr.name.clone(),
            typ: rust_type(&attr.typ).to_string(),
        });
    }
    for port in &part.ports {
        if !port.signals.is_empty() {
            // Typed port field: e.g., `button_out: ButtonMessage`
            let msg_name = port_message_name(&port.typ);
            fields.push(RustField {
                name: port.name.clone(),
                typ: msg_name,
            });
        }
    }
    items.push(RustItem::Struct(RustStruct {
        name: part.name.clone(),
        fields,
        derives: Vec::new(),
    }));

    // Impl block
    let mut methods = Vec::new();

    // new() method
    let mut field_defaults = Vec::new();
    for attr in &part.attributes {
        field_defaults.push((attr.name.clone(), rust_default(&attr.typ, &attr.default)));
    }
    for port in &part.ports {
        if !port.signals.is_empty() {
            let msg_name = port_message_name(&port.typ);
            let sig_defaults: Vec<String> = port.signals.iter()
                .map(|sig| format!("{}: {}", sig.name, rust_default(&sig.typ, &None)))
                .collect();
            let init_expr = format!("{msg_name} {{ {} }}", sig_defaults.join(", "));
            field_defaults.push((port.name.clone(), init_expr));
        }
    }

    let initial_state = match &part.state_machine {
        Some(sm) => {
            if !sm.states.is_empty() {
                let mut enum_name = String::new();
                enum_name.push_str(part.name.as_str());
                enum_name.push_str("State");
                let variant = to_pascal_case(&sm.states[0].name);
                Some((enum_name, variant))
            } else {
                None
            }
        }
        None => None,
    };

    methods.push(RustMethod {
        doc: None,
        name: "new".to_string(),
        params: String::new(),
        ret_type: Some("Self".to_string()),
        body: MethodBody::NewConstructor(NewBody {
            initial_state,
            field_defaults,
        }),
    });

    // step() method
    match &part.state_machine {
        Some(sm) => {
            if !sm.states.is_empty() {
                let step_body = translate_state_machine(sm, &part.name, &part.attributes);
                methods.push(RustMethod {
                    doc: Some("Execute one step of the state machine".to_string()),
                    name: "step".to_string(),
                    params: "&mut self".to_string(),
                    ret_type: None,
                    body: MethodBody::StepStateMachine(step_body),
                });
            }
        }
        None => {}
    }

    items.push(RustItem::Impl(RustImpl {
        type_name: part.name.clone(),
        methods,
    }));

    // Port trait impls
    for port in &part.ports {
        let mut port_def_opt: Option<&Port> = None;
        for pd in port_defs.iter() {
            if pd.typ == port.typ {
                port_def_opt = Some(pd);
                break;
            }
        }
        match port_def_opt {
            Some(port_def) => {
                let mut trait_methods = Vec::new();
                for sig in &port_def.signals {
                    let rtype = rust_type(&sig.typ).to_string();
                    match sig.direction {
                        SignalDirection::Out => {
                            // getter: self.port_name.signal_name
                            let get_body_line = format!(
                                "self.{}.{}", port.name, sig.name
                            );
                            trait_methods.push(RustMethod {
                                doc: None,
                                name: {
                                    let mut n = String::new();
                                    n.push_str("get_");
                                    n.push_str(sig.name.as_str());
                                    n
                                },
                                params: "&self".to_string(),
                                ret_type: Some(rtype.clone()),
                                body: MethodBody::SimpleLines(vec![get_body_line]),
                            });
                            // setter: self.port_name.signal_name = value;
                            let mut set_params = String::new();
                            set_params.push_str("&mut self, value: ");
                            set_params.push_str(rust_type(&sig.typ));
                            let set_body_line = format!(
                                "self.{}.{} = value;", port.name, sig.name
                            );
                            trait_methods.push(RustMethod {
                                doc: None,
                                name: {
                                    let mut n = String::new();
                                    n.push_str("set_");
                                    n.push_str(sig.name.as_str());
                                    n
                                },
                                params: set_params,
                                ret_type: None,
                                body: MethodBody::SimpleLines(vec![set_body_line]),
                            });
                        }
                        SignalDirection::In => {
                            let get_body_line = format!(
                                "self.{}.{}", port.name, sig.name
                            );
                            trait_methods.push(RustMethod {
                                doc: None,
                                name: {
                                    let mut n = String::new();
                                    n.push_str("get_");
                                    n.push_str(sig.name.as_str());
                                    n
                                },
                                params: "&self".to_string(),
                                ret_type: Some(rtype),
                                body: MethodBody::SimpleLines(vec![get_body_line]),
                            });
                        }
                    }
                }
                items.push(RustItem::TraitImpl(RustTraitImpl {
                    trait_name: port.typ.clone(),
                    type_name: part.name.clone(),
                    methods: trait_methods,
                }));
            }
            None => {}
        }
    }

    items
}

/// Translate a state machine into a StepBody IR.
#[verifier::external_body]
fn translate_state_machine(
    sm: &StateMachine,
    part_name: &str,
    attributes: &[Attribute],
) -> StepBody {
    let mut state_enum = String::new();
    state_enum.push_str(part_name);
    state_enum.push_str("State");

    let mut arms = Vec::new();

    for state in &sm.states {
        let variant = to_pascal_case(&state.name);

        // Do actions
        let mut do_actions = Vec::new();
        for action in &state.do_actions {
            if let Some(body) = &action.body {
                let assignments = expr::parse_assignments(body);
                if !assignments.is_empty() {
                    let mut comment = String::new();
                    comment.push_str("Do action: ");
                    comment.push_str(action.name.as_str());

                    for (i, pair) in assignments.iter().enumerate() {
                        let rust_expr = expr::sysml_expr_to_rust(&pair.1, attributes);
                        do_actions.push(Assignment {
                            comment: if i == 0 { Some(comment.clone()) } else { None },
                            var: pair.0.clone(),
                            expr: rust_expr,
                        });
                    }
                }
            }
        }

        // Exit actions
        let mut exit_actions = Vec::new();
        for action in &state.exit_actions {
            if let Some(body) = &action.body {
                let assignments = expr::parse_assignments(body);
                if !assignments.is_empty() {
                    let mut comment = String::new();
                    comment.push_str("Exit action: ");
                    comment.push_str(action.name.as_str());

                    for (i, pair) in assignments.iter().enumerate() {
                        let rust_expr = expr::sysml_expr_to_rust(&pair.1, attributes);
                        exit_actions.push(Assignment {
                            comment: if i == 0 { Some(comment.clone()) } else { None },
                            var: pair.0.clone(),
                            expr: rust_expr,
                        });
                    }
                }
            }
        }

        // Transitions from this state
        let mut conditional: Vec<&Transition> = Vec::new();
        let mut unconditional: Vec<&Transition> = Vec::new();
        for t in sm.transitions.iter() {
            if t.from_state == state.name {
                if t.condition.is_some() {
                    conditional.push(t);
                } else {
                    unconditional.push(t);
                }
            }
        }

        let mut transitions = Vec::new();
        if !conditional.is_empty() || !unconditional.is_empty() {
            for trans in conditional.iter() {
                let to_variant = to_pascal_case(&trans.to_state);
                let cond_str = match &trans.condition {
                    Some(c) => c.as_str(),
                    None => "true",
                };
                let cond = expr::sysml_condition_to_rust(cond_str, attributes);
                let transition_actions = collect_transition_actions_rust(trans, attributes);
                let entry_actions = collect_entry_actions(sm, &trans.to_state, attributes);
                transitions.push(TransitionCode::Conditional {
                    condition: cond,
                    target_variant: to_variant,
                    transition_actions,
                    entry_actions,
                });
            }
            if !conditional.is_empty() && !unconditional.is_empty() {
                let trans = unconditional[0];
                let to_variant = to_pascal_case(&trans.to_state);
                let transition_actions = collect_transition_actions_rust(trans, attributes);
                let entry_actions = collect_entry_actions(sm, &trans.to_state, attributes);
                transitions.push(TransitionCode::Unconditional {
                    target_variant: to_variant,
                    transition_actions,
                    entry_actions,
                });
            } else {
                for trans in &unconditional {
                    let to_variant = to_pascal_case(&trans.to_state);
                    let transition_actions = collect_transition_actions_rust(trans, attributes);
                    let entry_actions = collect_entry_actions(sm, &trans.to_state, attributes);
                    transitions.push(TransitionCode::Unconditional {
                        target_variant: to_variant,
                        transition_actions,
                        entry_actions,
                    });
                }
            }
        }

        arms.push(StepArm {
            variant,
            do_actions,
            exit_actions,
            transitions,
        });
    }

    StepBody { state_enum, arms }
}

/// Collect entry actions for a target state.
#[verifier::external_body]
fn collect_entry_actions(
    sm: &StateMachine,
    target_state_name: &str,
    attributes: &[Attribute],
) -> Vec<Assignment> {
    let mut result = Vec::new();
    let target_opt = sm.states.iter().find(|s| s.name == target_state_name);
    if let Some(target) = target_opt {
        for action in &target.entry_actions {
            if let Some(body) = &action.body {
                let assignments = expr::parse_assignments(body);
                if !assignments.is_empty() {
                    let mut comment = String::new();
                    comment.push_str("Entry action for ");
                    comment.push_str(&to_pascal_case(target_state_name));
                    comment.push_str(": ");
                    comment.push_str(action.name.as_str());

                    for (i, pair) in assignments.iter().enumerate() {
                        let rust_expr = expr::sysml_expr_to_rust(&pair.1, attributes);
                        result.push(Assignment {
                            comment: if i == 0 { Some(comment.clone()) } else { None },
                            var: pair.0.clone(),
                            expr: rust_expr,
                        });
                    }
                }
            }
        }
    }
    result
}

/// Collect transition actions for a transition.
#[verifier::external_body]
fn collect_transition_actions_rust(
    trans: &Transition,
    attributes: &[Attribute],
) -> Vec<Assignment> {
    let mut result = Vec::new();
    for action in &trans.actions {
        if let Some(body) = &action.body {
            let assignments = expr::parse_assignments(body);
            if !assignments.is_empty() {
                let mut comment = String::new();
                comment.push_str("Transition action: ");
                comment.push_str(action.name.as_str());

                for (i, pair) in assignments.iter().enumerate() {
                    let rust_expr = expr::sysml_expr_to_rust(&pair.1, attributes);
                    result.push(Assignment {
                        comment: if i == 0 { Some(comment.clone()) } else { None },
                        var: pair.0.clone(),
                        expr: rust_expr,
                    });
                }
            }
        }
    }
    result
}

} // verus!
