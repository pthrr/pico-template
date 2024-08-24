//! `SysML` AST types.
//!
//! Intermediate representation between parsed `SysML` text and generated Rust code.

/// A `SysML` attribute (field on a part).
#[derive(Clone, Debug)]
pub struct Attribute {
    pub name: String,
    pub typ: String,
    pub default: Option<String>,
}

/// A signal within a port definition.
#[derive(Clone, Debug)]
pub struct Signal {
    pub name: String,
    pub typ: String,
    pub direction: SignalDirection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalDirection {
    In,
    Out,
}

/// A port definition.
#[derive(Clone, Debug)]
pub struct Port {
    pub name: String,
    pub typ: String,
    pub signals: Vec<Signal>,
    pub attributes: Vec<Attribute>,
}

/// An action attached to a state (entry, do, exit).
#[derive(Clone, Debug)]
pub struct StateAction {
    pub name: String,
    pub body: Option<String>,
    /// Worst-case execution time in microseconds (from `wcet N;` annotation).
    pub wcet_us: Option<u64>,
}

/// A state in a state machine.
#[derive(Clone, Debug)]
pub struct State {
    pub name: String,
    pub doc: Option<String>,
    pub entry_actions: Vec<StateAction>,
    pub do_actions: Vec<StateAction>,
    pub exit_actions: Vec<StateAction>,
}

/// A transition between states.
#[derive(Clone, Debug)]
pub struct Transition {
    pub from_state: String,
    pub to_state: String,
    pub condition: Option<String>,
    pub is_accept: bool,
    /// The accept variable name (env input), separate from guard condition.
    pub accept_var: Option<String>,
    /// Actions executed during the transition (between exit and entry).
    pub actions: Vec<StateAction>,
}

/// A state machine (set of states + transitions).
#[derive(Clone, Debug)]
pub struct StateMachine {
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
}

/// A package-level action definition.
#[derive(Clone, Debug)]
pub struct ActionDef {
    pub name: String,
    pub body: String,
    /// Worst-case execution time in microseconds (from `wcet N;` annotation).
    pub wcet_us: Option<u64>,
}

/// An enum definition.
#[derive(Clone, Debug)]
pub struct EnumDef {
    pub name: String,
    pub values: Vec<String>,
}

/// A group of correlated input variables that change atomically.
#[derive(Clone, Debug)]
pub struct InputGroup {
    pub name: String,
    pub members: Vec<String>,
}

/// A part instance inside a system-level part def.
#[derive(Clone, Debug)]
pub struct PartInstance {
    pub name: String,
    pub typ: String,
}

/// A connection between two ports via part instances.
#[derive(Clone, Debug)]
pub struct Connection {
    pub from_part: String,
    pub from_port: String,
    pub to_part: String,
    pub to_port: String,
    pub capacity: usize,
}

/// A part definition (struct with optional state machine).
#[derive(Clone, Debug)]
pub struct PartDef {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub ports: Vec<Port>,
    pub state_machine: Option<StateMachine>,
    /// Correlated input groups for atomic Env actions.
    pub input_groups: Vec<InputGroup>,
    /// Part instances (for system-level composition).
    pub part_instances: Vec<PartInstance>,
    /// Connections between part instance ports.
    pub connections: Vec<Connection>,
}

/// Top-level package.
#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub enums: Vec<EnumDef>,
    pub port_defs: Vec<Port>,
    pub action_defs: Vec<ActionDef>,
    pub state_defs: Vec<State>,
    pub parts: Vec<PartDef>,
}

impl Package {
    /// True when the package only describes system wiring (no actor state machines).
    /// Rust for these packages is limited to `channels.rs` (emitted in the composition phase).
    pub fn is_composition_only(&self) -> bool {
        self.state_defs.is_empty()
            && !self.parts.is_empty()
            && self.parts.iter().all(|p| !p.part_instances.is_empty())
    }
}
