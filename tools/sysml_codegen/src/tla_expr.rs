//! SysML expression → TLA+ expression translation.
//!
//! Trusted boundary: string manipulation for converting SysML expressions
//! to TLA+ syntax. Unlike `expr.rs` (Rust path), TLA+ uses bare variable
//! names (no `self.` prefix) and different boolean/logical operators.

/// Convert a SysML expression to TLA+ syntax.
pub fn sysml_expr_to_tla(expr: &str) -> String {
    let expr = expr.trim();

    if expr.eq_ignore_ascii_case("true") {
        return "TRUE".to_string();
    }
    if expr.eq_ignore_ascii_case("false") {
        return "FALSE".to_string();
    }
    // Numeric: strip trailing .0 for integer approximation
    if let Ok(f) = expr.parse::<f64>() {
        #[allow(clippy::cast_possible_truncation)]
        let i = f as i64;
        #[allow(clippy::float_cmp)]
        if f == i as f64 {
            return i.to_string();
        }
        return i.to_string();
    }
    if let Some(rest) = expr.strip_prefix("not ") {
        let inner = sysml_expr_to_tla(rest.trim());
        return format!("~{inner}");
    }

    // Replace logical operators
    let result = expr.replace(" and ", " /\\ ").replace(" or ", " \\/ ");
    result
}

/// Convert a SysML condition to TLA+ syntax.
pub fn sysml_condition_to_tla(condition: &str) -> String {
    let result = condition
        .trim()
        .replace(" and ", " /\\ ")
        .replace(" or ", " \\/ ")
        .replace(" not ", " ~");
    // Replace >= and <= with TLA+ equivalents (same syntax, works as-is)
    result
}

/// Parse `:=` assignment lines from an action body (same logic as expr.rs).
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
