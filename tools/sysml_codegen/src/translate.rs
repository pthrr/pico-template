//! Translation: `SysML` AST → typed Rust IR.
//!

use crate::ast::{
    Attribute, EnumDef, Package, PartDef, Port, SignalDirection, StateMachine, Transition,
};
use crate::expr;
use crate::rust_ast::{
    Assignment, MethodBody, NewBody, RustEnum, RustField, RustImpl, RustItem, RustMethod,
    RustModule, RustStruct, RustTrait, RustTraitImpl, RustVariant, StepArm, StepBody,
    TraitMethodSig, TransitionCode,
};

/// Convert `snake_case` to `PascalCase`.
pub fn to_pascal_case(s: &str) -> String {
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

/// Map `SysML` type names to Rust type names.
pub fn rust_type(sysml_type: &str) -> &'static str {
    match sysml_type {
        "Real" => "f32",
        "Integer" => "i32",
        "Boolean" => "bool",
        "String" => "&str",
        _ => "/* unknown */",
    }
}

/// Get Rust default value for a `SysML` type.
pub fn rust_default(sysml_type: &str, default: Option<&str>) -> String {
    if let Some(val) = default {
        return match sysml_type {
            "Real" => {
                if val.contains('.') {
                    val.to_string()
                } else {
                    format!("{val}.0")
                }
            }
            "Boolean" => val.to_lowercase(),
            _ => val.to_string(),
        };
    }
    match sysml_type {
        "Real" => "0.0".to_string(),
        "Integer" => "0".to_string(),
        "Boolean" => "false".to_string(),
        "String" => "\"\"".to_string(),
        _ => "Default::default()".to_string(),
    }
}

/// Translate a `SysML` Package into a typed Rust IR module.
pub fn translate(package: &Package) -> RustModule {
    let mut items: Vec<RustItem> = Vec::new();

    // Header comments
    items.push(RustItem::Comment(
        "Auto-generated from SysML 2 model".to_string(),
    ));
    let mut pkg_comment = String::new();
    pkg_comment.push_str("Package: ");
    pkg_comment.push_str(package.name.as_str());
    items.push(RustItem::Comment(pkg_comment));
    items.push(RustItem::BlankLine);

    // Enums
    for action_def in &package.action_defs {
        items.push(RustItem::Comment(format!("action def {}", action_def.name)));
    }
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

/// Translate a `SysML` enum into a Rust IR enum.
pub fn translate_enum(e: &EnumDef) -> RustEnum {
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
fn port_message_name(port_type: &str) -> String {
    if let Some(base) = port_type.strip_suffix("Port") {
        format!("{base}Message")
    } else {
        format!("{port_type}Message")
    }
}

/// Generate a message struct from a port definition's signals.
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
        derives: vec!["Debug".to_string(), "Clone".to_string(), "Copy".to_string()],
    }
}

/// Translate a `SysML` part into a set of Rust IR items.
pub fn translate_part(part: &PartDef, port_defs: &[Port]) -> Vec<RustItem> {
    let mut items = translate_part_types(part);
    items.push(RustItem::BlankLine);

    // Impl block
    let mut methods = Vec::new();

    // new() method
    let mut field_defaults = Vec::new();
    for attr in &part.attributes {
        field_defaults.push((
            attr.name.clone(),
            rust_default(&attr.typ, attr.default.as_deref()),
        ));
    }
    for port in &part.ports {
        if !port.signals.is_empty() {
            let msg_name = port_message_name(&port.typ);
            let sig_defaults: Vec<String> = port
                .signals
                .iter()
                .map(|sig| format!("{}: {}", sig.name, rust_default(&sig.typ, None)))
                .collect();
            let init_expr = format!("{msg_name} {{ {} }}", sig_defaults.join(", "));
            field_defaults.push((port.name.clone(), init_expr));
        }
    }

    let initial_state = match &part.state_machine {
        Some(sm) => {
            if sm.states.is_empty() {
                None
            } else {
                let mut enum_name = String::new();
                enum_name.push_str(part.name.as_str());
                enum_name.push_str("State");
                let variant = to_pascal_case(&sm.states[0].name);
                Some((enum_name, variant))
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
    if let Some(sm) = &part.state_machine {
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

    items.push(RustItem::Impl(RustImpl {
        type_name: part.name.clone(),
        methods,
    }));

    items.extend(translate_port_trait_impls(part, port_defs));
    items
}

fn translate_part_types(part: &PartDef) -> Vec<RustItem> {
    let mut items = vec![RustItem::Comment(part.name.clone()), RustItem::BlankLine];
    if let Some(sm) = &part.state_machine {
        let state_enum_name = format!("{}State", part.name);
        let variants = sm
            .states
            .iter()
            .map(|state| RustVariant {
                name: to_pascal_case(&state.name),
                doc: state.doc.clone(),
            })
            .collect();
        items.push(RustItem::Enum(RustEnum {
            name: state_enum_name,
            variants,
        }));
    }

    let mut fields = Vec::new();
    if part.state_machine.is_some() {
        fields.push(RustField {
            name: "state".to_string(),
            typ: format!("{}State", part.name),
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
            fields.push(RustField {
                name: port.name.clone(),
                typ: port_message_name(&port.typ),
            });
        }
    }
    items.push(RustItem::Struct(RustStruct {
        name: part.name.clone(),
        fields,
        derives: Vec::new(),
    }));
    items
}

fn translate_port_trait_impls(part: &PartDef, port_defs: &[Port]) -> Vec<RustItem> {
    let mut items = Vec::new();
    for port in &part.ports {
        let Some(port_def) = port_defs.iter().find(|pd| pd.typ == port.typ) else {
            continue;
        };
        let mut trait_methods = Vec::new();
        for sig in &port_def.signals {
            let rtype = rust_type(&sig.typ).to_string();
            match sig.direction {
                SignalDirection::Out => {
                    let get_body_line = format!("self.{}.{}", port.name, sig.name);
                    trait_methods.push(RustMethod {
                        doc: None,
                        name: format!("get_{}", sig.name),
                        params: "&self".to_string(),
                        ret_type: Some(rtype.clone()),
                        body: MethodBody::SimpleLines(vec![get_body_line]),
                    });
                    let set_body_line = format!("self.{}.{} = value;", port.name, sig.name);
                    trait_methods.push(RustMethod {
                        doc: None,
                        name: format!("set_{}", sig.name),
                        params: format!("&mut self, value: {}", rust_type(&sig.typ)),
                        ret_type: None,
                        body: MethodBody::SimpleLines(vec![set_body_line]),
                    });
                }
                SignalDirection::In => {
                    let get_body_line = format!("self.{}.{}", port.name, sig.name);
                    trait_methods.push(RustMethod {
                        doc: None,
                        name: format!("get_{}", sig.name),
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
    items
}

/// Translate a state machine into a `StepBody` IR.
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
        arms.push(translate_state_step_arm(state, sm, attributes));
    }

    StepBody { state_enum, arms }
}

fn translate_state_step_arm(
    state: &crate::ast::State,
    sm: &StateMachine,
    attributes: &[Attribute],
) -> StepArm {
    let variant = to_pascal_case(&state.name);
    let do_actions = state_actions_to_assignments(&state.do_actions, "Do action: ", attributes);
    let exit_actions =
        state_actions_to_assignments(&state.exit_actions, "Exit action: ", attributes);
    let transitions = transitions_for_state(state, sm, attributes);
    StepArm {
        variant,
        do_actions,
        exit_actions,
        transitions,
    }
}

fn state_actions_to_assignments(
    actions: &[crate::ast::StateAction],
    label: &str,
    attributes: &[Attribute],
) -> Vec<Assignment> {
    let mut result = Vec::new();
    for action in actions {
        let Some(body) = &action.body else {
            continue;
        };
        let assignments = expr::parse_assignments(body);
        if assignments.is_empty() {
            continue;
        }
        let comment = format!("{label}{}", action.name);
        for (i, pair) in assignments.iter().enumerate() {
            result.push(Assignment {
                comment: if i == 0 { Some(comment.clone()) } else { None },
                var: pair.0.clone(),
                expr: expr::sysml_expr_to_rust(&pair.1, attributes),
            });
        }
    }
    result
}

fn transitions_for_state(
    state: &crate::ast::State,
    sm: &StateMachine,
    attributes: &[Attribute],
) -> Vec<TransitionCode> {
    let mut conditional = Vec::new();
    let mut unconditional = Vec::new();
    for t in &sm.transitions {
        if t.from_state == state.name {
            if t.condition.is_some() {
                conditional.push(t);
            } else {
                unconditional.push(t);
            }
        }
    }

    let mut transitions = Vec::new();
    if conditional.is_empty() && unconditional.is_empty() {
        return transitions;
    }

    for trans in &conditional {
        let to_variant = to_pascal_case(&trans.to_state);
        let cond_str = trans.condition.as_deref().unwrap_or("true");
        transitions.push(TransitionCode::Conditional {
            condition: expr::sysml_condition_to_rust(cond_str, attributes),
            target_variant: to_variant,
            transition_actions: collect_transition_actions_rust(trans, attributes),
            entry_actions: collect_entry_actions(sm, &trans.to_state, attributes),
        });
    }

    if !conditional.is_empty() && !unconditional.is_empty() {
        let trans = unconditional[0];
        transitions.push(TransitionCode::Unconditional {
            target_variant: to_pascal_case(&trans.to_state),
            transition_actions: collect_transition_actions_rust(trans, attributes),
            entry_actions: collect_entry_actions(sm, &trans.to_state, attributes),
        });
    } else {
        for trans in &unconditional {
            transitions.push(TransitionCode::Unconditional {
                target_variant: to_pascal_case(&trans.to_state),
                transition_actions: collect_transition_actions_rust(trans, attributes),
                entry_actions: collect_entry_actions(sm, &trans.to_state, attributes),
            });
        }
    }
    transitions
}

/// Collect entry actions for a target state.
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
