//! Typed Rust IR — models the structure of generated Rust code.
//!
//! All types live inside `verus!{}` so the translation from SysML AST
//! to this IR can be formally verified.

use vstd::prelude::*;

verus! {

/// A complete generated Rust module.
pub struct RustModule {
    pub package_name: String,
    pub items: Vec<RustItem>,
}

/// A top-level item in the generated Rust code.
pub enum RustItem {
    Enum(RustEnum),
    Trait(RustTrait),
    Struct(RustStruct),
    Impl(RustImpl),
    TraitImpl(RustTraitImpl),
    Comment(String),
    BlankLine,
}

/// A Rust enum definition.
pub struct RustEnum {
    pub name: String,
    pub variants: Vec<RustVariant>,
}

/// A variant within a Rust enum.
pub struct RustVariant {
    pub name: String,
    pub doc: Option<String>,
}

/// A Rust trait definition.
pub struct RustTrait {
    pub name: String,
    pub methods: Vec<TraitMethodSig>,
}

/// A method signature inside a trait.
pub struct TraitMethodSig {
    pub name: String,
    pub params: String,
    pub ret_type: String,
}

/// A Rust struct definition.
pub struct RustStruct {
    pub name: String,
    pub fields: Vec<RustField>,
    /// Extra derive macros (e.g., ["Debug", "Clone", "Copy"] for message structs).
    pub derives: Vec<String>,
}

/// A field in a Rust struct.
pub struct RustField {
    pub name: String,
    pub typ: String,
}

/// An inherent `impl` block.
pub struct RustImpl {
    pub type_name: String,
    pub methods: Vec<RustMethod>,
}

/// A trait `impl` block.
pub struct RustTraitImpl {
    pub trait_name: String,
    pub type_name: String,
    pub methods: Vec<RustMethod>,
}

/// A method inside an `impl` block.
pub struct RustMethod {
    pub doc: Option<String>,
    pub name: String,
    pub params: String,
    pub ret_type: Option<String>,
    pub body: MethodBody,
}

/// The body of a method.
pub enum MethodBody {
    /// `fn new() -> Self { Self { ... } }`
    NewConstructor(NewBody),
    /// `fn step(&mut self) { match self.state { ... } }`
    StepStateMachine(StepBody),
    /// Simple getter/setter lines.
    SimpleLines(Vec<String>),
}

/// Body of a `new()` constructor.
pub struct NewBody {
    /// Optional initial state: (EnumName, VariantName).
    pub initial_state: Option<(String, String)>,
    /// Field defaults: (field_name, default_value_expr).
    pub field_defaults: Vec<(String, String)>,
}

/// Body of a `step()` state machine method.
pub struct StepBody {
    pub state_enum: String,
    pub arms: Vec<StepArm>,
}

/// One arm of the state machine match.
pub struct StepArm {
    pub variant: String,
    pub do_actions: Vec<Assignment>,
    pub exit_actions: Vec<Assignment>,
    pub transitions: Vec<TransitionCode>,
}

/// A variable assignment.
pub struct Assignment {
    pub comment: Option<String>,
    pub var: String,
    pub expr: String,
}

/// A transition within a state machine arm.
pub enum TransitionCode {
    Conditional {
        condition: String,
        target_variant: String,
        transition_actions: Vec<Assignment>,
        entry_actions: Vec<Assignment>,
    },
    Unconditional {
        target_variant: String,
        transition_actions: Vec<Assignment>,
        entry_actions: Vec<Assignment>,
    },
}

} // verus!
