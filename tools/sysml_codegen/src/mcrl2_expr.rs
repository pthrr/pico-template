//! `SysML` expression → mCRL2 expression translation.
//!
//! Trusted boundary: string manipulation for converting `SysML` expressions
//! to mCRL2 syntax. mCRL2 uses lowercase `true`/`false`, `!` for not,
//! `&&`/`||` for and/or, and standard comparison operators.

/// Convert a `SysML` expression to mCRL2 syntax.
pub fn sysml_expr_to_mcrl2(expr: &str) -> String {
    let expr = expr.trim();

    if expr.eq_ignore_ascii_case("true") {
        return "true".to_string();
    }
    if expr.eq_ignore_ascii_case("false") {
        return "false".to_string();
    }
    if let Ok(i) = expr.parse::<i64>() {
        return i.to_string();
    }
    if let Some(stripped) = expr.strip_suffix(".0") {
        if let Ok(i) = stripped.parse::<i64>() {
            return i.to_string();
        }
    }
    if let Some(rest) = expr.strip_prefix("not ") {
        let inner = sysml_expr_to_mcrl2(rest.trim());
        return format!("!({inner})");
    }

    // Replace logical operators

    expr.replace(" and ", " && ").replace(" or ", " || ")
}

/// Convert a `SysML` condition to mCRL2 syntax.
pub fn sysml_condition_to_mcrl2(condition: &str) -> String {
    let result = condition
        .trim()
        .replace(" and ", " && ")
        .replace(" or ", " || ")
        .replace(" not ", " !");
    result
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_true_false() {
        assert_eq!(sysml_expr_to_mcrl2("true"), "true");
        assert_eq!(sysml_expr_to_mcrl2("TRUE"), "true");
        assert_eq!(sysml_expr_to_mcrl2("True"), "true");
        assert_eq!(sysml_expr_to_mcrl2("false"), "false");
        assert_eq!(sysml_expr_to_mcrl2("FALSE"), "false");
    }

    #[test]
    fn test_expr_numbers() {
        assert_eq!(sysml_expr_to_mcrl2("42"), "42");
        assert_eq!(sysml_expr_to_mcrl2("3.0"), "3");
        assert_eq!(sysml_expr_to_mcrl2("0"), "0");
        assert_eq!(sysml_expr_to_mcrl2("0.0"), "0");
    }

    #[test]
    fn test_expr_not() {
        assert_eq!(sysml_expr_to_mcrl2("not x"), "!(x)");
        assert_eq!(sysml_expr_to_mcrl2("not true"), "!(true)");
        assert_eq!(sysml_expr_to_mcrl2("not false"), "!(false)");
    }

    #[test]
    fn test_expr_and_or() {
        assert_eq!(sysml_expr_to_mcrl2("a and b"), "a && b");
        assert_eq!(sysml_expr_to_mcrl2("a or b"), "a || b");
    }

    #[test]
    fn test_condition() {
        assert_eq!(sysml_condition_to_mcrl2("x and not y"), "x && !y");
        assert_eq!(sysml_condition_to_mcrl2("a or b"), "a || b");
        assert_eq!(sysml_condition_to_mcrl2("  a and b  "), "a && b");
    }

    #[test]
    fn test_parse_assignments_single() {
        let result = parse_assignments("x := 42;");
        assert_eq!(result, vec![("x".to_string(), "42".to_string())]);
    }

    #[test]
    fn test_parse_assignments_multiple() {
        let result = parse_assignments("x := 1;\ny := 2;");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], ("x".to_string(), "1".to_string()));
        assert_eq!(result[1], ("y".to_string(), "2".to_string()));
    }

    #[test]
    fn test_parse_assignments_comments_and_empty() {
        let result = parse_assignments("// comment\nx := 1;");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "x");

        let result = parse_assignments("\n\nx := 1;\n\n");
        assert_eq!(result.len(), 1);

        assert!(parse_assignments("").is_empty());
    }

    #[test]
    fn test_parse_assignments_no_semicolon() {
        let result = parse_assignments("x := 1");
        assert_eq!(result, vec![("x".to_string(), "1".to_string())]);
    }
}
