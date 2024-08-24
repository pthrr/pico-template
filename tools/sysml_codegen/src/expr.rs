//! Expression string helpers for converting `SysML` expressions to Rust.
//!
//! Pure string manipulation (`str::find`, `replace`, indexing, `parse`).

use crate::ast::Attribute;

/// Parse `:=` assignment lines from an action body.
pub fn parse_assignments(body: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if !line.is_empty() && !line.starts_with("//") {
            if let Some(idx) = line.find(":=") {
                let var = line[..idx].trim().to_string();
                let trimmed = line[idx + 2..].trim();
                let end = trimmed.trim_end_matches(';').len();
                let expr = trimmed[..end].trim().to_string();
                if !var.is_empty() && !expr.is_empty() {
                    result.push((var, expr));
                }
            }
        }
    }
    result
}

/// Convert a `SysML` expression to Rust syntax.
pub fn sysml_expr_to_rust(expr: &str, attributes: &[Attribute]) -> String {
    let expr = expr.trim();

    if expr.eq_ignore_ascii_case("true") {
        return "true".to_string();
    }
    if expr.eq_ignore_ascii_case("false") {
        return "false".to_string();
    }
    if expr.parse::<f64>().is_ok() {
        return expr.to_string();
    }
    if let Some(rest) = expr.strip_prefix("not ") {
        let inner = replace_attr_refs(rest.trim(), attributes);
        return format!("!{inner}");
    }

    let result = expr
        .replace(" and ", " && ")
        .replace(" or ", " || ")
        .replace(" mod ", " % ");
    replace_attr_refs(&result, attributes)
}

/// Convert a `SysML` condition to Rust syntax.
pub fn sysml_condition_to_rust(condition: &str, attributes: &[Attribute]) -> String {
    let result = condition
        .trim()
        .replace(" and ", " && ")
        .replace(" or ", " || ")
        .replace(" not ", " !")
        .replace(" mod ", " % ");
    replace_attr_refs(&result, attributes)
}

/// Replace bare attribute references with `self.attr` form.
fn replace_attr_refs(expr: &str, attributes: &[Attribute]) -> String {
    let mut result = expr.to_string();
    for attr in attributes {
        let pattern = &attr.name;
        let replacement = format!("self.{}", attr.name);
        let mut new_result = String::new();
        let mut remaining = result.as_str();

        loop {
            match remaining.find(pattern.as_str()) {
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
                    let already_self = pos >= 5 && &remaining[pos - 5..pos] == "self.";

                    if before_ok && after_ok && !already_self {
                        new_result.push_str(&remaining[..pos]);
                        new_result.push_str(&replacement);
                        remaining = &remaining[after_pos..];
                    } else {
                        new_result.push_str(&remaining[..after_pos]);
                        remaining = &remaining[after_pos..];
                    }
                }
            }
        }
        new_result.push_str(remaining);
        result = new_result;
    }
    result
}
