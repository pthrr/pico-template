//! Pretty printer: Rust IR → String.
//!
//! Trusted boundary — mechanical `format!`-based rendering.
//! The entire module is `#[verifier::external_body]` since it only
//! does string concatenation with `format!`.

use crate::rust_ast::*;
use vstd::prelude::*;

verus! {

/// Render a `RustModule` into a complete Rust source string.
#[verifier::external_body]
pub fn render(module: &RustModule) -> (result: String)
    ensures result@.len() > 0,
{
    let mut buf: Vec<String> = Vec::new();

    for item in &module.items {
        match item {
            RustItem::Comment(text) => buf.push(format!("// {text}")),
            RustItem::BlankLine => buf.push(String::new()),
            RustItem::Enum(e) => render_enum(&mut buf, e),
            RustItem::Trait(t) => render_trait(&mut buf, t),
            RustItem::Struct(s) => render_struct(&mut buf, s),
            RustItem::Impl(im) => render_impl(&mut buf, im),
            RustItem::TraitImpl(ti) => render_trait_impl(&mut buf, ti),
        }
    }

    let mut result = String::new();
    for (i, line) in buf.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        result.push_str(line.as_str());
    }
    result
}

} // verus!

fn render_enum(buf: &mut Vec<String>, e: &RustEnum) {
    buf.push("#[derive(Debug, Clone, Copy, PartialEq, Eq)]".to_string());
    buf.push(format!("pub enum {} {{", e.name));
    for v in &e.variants {
        if let Some(doc) = &v.doc {
            buf.push(format!("    /// {doc}"));
        }
        buf.push(format!("    {},", v.name));
    }
    buf.push("}".to_string());
    buf.push(String::new());
}

fn render_trait(buf: &mut Vec<String>, t: &RustTrait) {
    buf.push(format!("pub trait {} {{", t.name));
    for m in &t.methods {
        if m.ret_type.is_empty() {
            buf.push(format!("    fn {}({});", m.name, m.params));
        } else {
            buf.push(format!("    fn {}({}) -> {};", m.name, m.params, m.ret_type));
        }
    }
    buf.push("}".to_string());
    buf.push(String::new());
}

fn render_struct(buf: &mut Vec<String>, s: &RustStruct) {
    if !s.derives.is_empty() {
        buf.push(format!("#[derive({})]", s.derives.join(", ")));
    }
    buf.push(format!("pub struct {} {{", s.name));
    for f in &s.fields {
        buf.push(format!("    pub {}: {},", f.name, f.typ));
    }
    buf.push("}".to_string());
    buf.push(String::new());
}

fn render_impl(buf: &mut Vec<String>, im: &RustImpl) {
    buf.push(format!("impl {} {{", im.type_name));
    for (i, method) in im.methods.iter().enumerate() {
        render_method(buf, method, "    ");
        if i < im.methods.len() - 1 {
            buf.push(String::new());
        }
    }
    buf.push("}".to_string());
    buf.push(String::new());

    // Emit Default impl if there's a new() constructor
    let has_new = im.methods.iter().any(|m| {
        m.name == "new" && m.params.is_empty() && matches!(m.body, MethodBody::NewConstructor(_))
    });
    if has_new {
        buf.push(format!("impl Default for {} {{", im.type_name));
        buf.push("    fn default() -> Self {".to_string());
        buf.push("        Self::new()".to_string());
        buf.push("    }".to_string());
        buf.push("}".to_string());
        buf.push(String::new());
    }
}

fn render_trait_impl(buf: &mut Vec<String>, ti: &RustTraitImpl) {
    buf.push(format!("impl {} for {} {{", ti.trait_name, ti.type_name));
    for method in &ti.methods {
        render_method(buf, method, "    ");
    }
    buf.push("}".to_string());
    buf.push(String::new());
}

fn render_method(buf: &mut Vec<String>, method: &RustMethod, indent: &str) {
    if let Some(doc) = &method.doc {
        buf.push(format!("{indent}/// {doc}"));
    }
    if method.name == "new" && matches!(method.body, MethodBody::NewConstructor(_)) {
        buf.push(format!("{indent}#[must_use]"));
    }
    match &method.ret_type {
        Some(rt) => {
            buf.push(format!("{indent}pub fn {}({}) -> {rt} {{", method.name, method.params));
        }
        None => {
            buf.push(format!("{indent}pub fn {}({}) {{", method.name, method.params));
        }
    }
    match &method.body {
        MethodBody::NewConstructor(body) => render_new_body(buf, body, indent),
        MethodBody::StepStateMachine(body) => render_step_body(buf, body, indent),
        MethodBody::SimpleLines(lines) => {
            for line in lines {
                buf.push(format!("{indent}    {line}"));
            }
        }
    }
    buf.push(format!("{indent}}}"));
}

fn render_new_body(buf: &mut Vec<String>, body: &NewBody, indent: &str) {
    buf.push(format!("{indent}    Self {{"));
    if let Some((enum_name, variant)) = &body.initial_state {
        buf.push(format!("{indent}        state: {enum_name}::{variant},"));
    }
    for (name, value) in &body.field_defaults {
        buf.push(format!("{indent}        {name}: {value},"));
    }
    buf.push(format!("{indent}    }}"));
}

fn render_step_body(buf: &mut Vec<String>, body: &StepBody, indent: &str) {
    buf.push(format!("{indent}    match self.state {{"));

    for arm in &body.arms {
        buf.push(format!(
            "{indent}        {}::{} => {{",
            body.state_enum, arm.variant
        ));

        // Do actions
        for action in &arm.do_actions {
            if let Some(comment) = &action.comment {
                buf.push(format!("{indent}            // {comment}"));
            }
            buf.push(format!(
                "{indent}            {}",
                render_assignment(&action.var, &action.expr)
            ));
        }

        // Exit actions
        for action in &arm.exit_actions {
            if let Some(comment) = &action.comment {
                buf.push(format!("{indent}            // {comment}"));
            }
            buf.push(format!(
                "{indent}            {}",
                render_assignment(&action.var, &action.expr)
            ));
        }

        // Transitions
        if !arm.transitions.is_empty() {
            buf.push(String::new());
            buf.push(format!("{indent}            // Transitions"));

            // Check if we have conditional transitions
            let has_conditional = arm.transitions.iter().any(|t| {
                matches!(t, TransitionCode::Conditional { .. })
            });

            if has_conditional {
                let mut is_first = true;
                for trans in &arm.transitions {
                    match trans {
                        TransitionCode::Conditional {
                            condition,
                            target_variant,
                            transition_actions,
                            entry_actions,
                        } => {
                            if is_first {
                                buf.push(format!(
                                    "{indent}            if {condition} {{"
                                ));
                                is_first = false;
                            } else {
                                buf.push(format!(
                                    "{indent}            }} else if {condition} {{"
                                ));
                            }
                            render_entry_and_transition(
                                buf,
                                &body.state_enum,
                                target_variant,
                                transition_actions,
                                entry_actions,
                                &format!("{indent}                "),
                            );
                        }
                        TransitionCode::Unconditional {
                            target_variant,
                            transition_actions,
                            entry_actions,
                        } => {
                            buf.push(format!("{indent}            }} else {{"));
                            render_entry_and_transition(
                                buf,
                                &body.state_enum,
                                target_variant,
                                transition_actions,
                                entry_actions,
                                &format!("{indent}                "),
                            );
                        }
                    }
                }
                buf.push(format!("{indent}            }}"));
            } else {
                // Only unconditional transitions
                for trans in &arm.transitions {
                    if let TransitionCode::Unconditional {
                        target_variant,
                        transition_actions,
                        entry_actions,
                    } = trans
                    {
                        render_entry_and_transition(
                            buf,
                            &body.state_enum,
                            target_variant,
                            transition_actions,
                            entry_actions,
                            &format!("{indent}            "),
                        );
                    }
                }
            }
        }

        buf.push(format!("{indent}        }}"));
    }

    buf.push(format!("{indent}    }}"));
}

fn render_entry_and_transition(
    buf: &mut Vec<String>,
    state_enum: &str,
    target_variant: &str,
    transition_actions: &[Assignment],
    entry_actions: &[Assignment],
    indent: &str,
) {
    // Transition actions (between exit and entry)
    for action in transition_actions {
        if let Some(comment) = &action.comment {
            buf.push(format!("{indent}// {comment}"));
        }
        buf.push(format!("{indent}{}", render_assignment(&action.var, &action.expr)));
    }
    // Entry actions
    for action in entry_actions {
        if let Some(comment) = &action.comment {
            buf.push(format!("{indent}// {comment}"));
        }
        buf.push(format!("{indent}{}", render_assignment(&action.var, &action.expr)));
    }
    buf.push(format!(
        "{indent}self.state = {state_enum}::{target_variant};"
    ));
}

/// Render a `self.var = expr;` assignment, using compound operators where possible.
///
/// Detects `self.var = self.var + X` → `self.var += X` (and `-=`).
fn render_assignment(var: &str, expr: &str) -> String {
    let self_var = format!("self.{var}");
    let plus_prefix = format!("{self_var} + ");
    let minus_prefix = format!("{self_var} - ");

    if let Some(rest) = expr.strip_prefix(&plus_prefix) {
        format!("self.{var} += {rest};")
    } else if let Some(rest) = expr.strip_prefix(&minus_prefix) {
        format!("self.{var} -= {rest};")
    } else {
        format!("self.{var} = {expr};")
    }
}
