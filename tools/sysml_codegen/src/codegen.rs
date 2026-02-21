//! Rust code generation from SysML AST.
//!
//! Thin wrapper: translate to typed IR, then render to string.

use crate::ast::Package;
use crate::render;
use crate::tla_render;
use crate::translate;

/// Generate a complete Rust source file from a SysML package.
pub fn generate(package: &Package) -> String {
    let module = translate::translate(package);
    render::render(&module)
}

/// Generate TLA+ specifications from a SysML package.
/// Returns `(part_name, tla_content, cfg_content)` per part with a state machine.
pub fn generate_tla(package: &Package) -> Vec<(String, String, String)> {
    tla_render::render_tla(package)
}
