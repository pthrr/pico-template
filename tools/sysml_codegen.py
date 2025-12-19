#!/usr/bin/env python3
"""
SysML 2 to Rust Code Generator

Pure SysML → Rust mapping:
- Enums → Rust enums
- Parts → Rust structs
- State machines → State enum + match-based transitions
- Attributes → Struct fields
- Ports → Trait definitions

No assumptions about:
- Runtime (Embassy, tokio, bare metal, etc.)
- Architecture (actor-based, event-driven, etc.)
- File structure (one file per actor, etc.)
"""

import re
from dataclasses import dataclass, field
from pathlib import Path


def to_pascal_case(snake_str: str) -> str:
    """Convert snake_case to PascalCase for Rust enum variants"""
    return ''.join(word.capitalize() for word in snake_str.split('_'))


@dataclass
class Attribute:
    name: str
    type: str
    default: str | None = None


@dataclass
class Signal:
    name: str
    type: str
    direction: str  # 'in' or 'out'
    array_size: int | None = None


@dataclass
class Port:
    name: str
    type: str
    signals: list[Signal] = field(default_factory=list)
    conjugated: bool = False
    attributes: list[Attribute] = field(default_factory=list)


@dataclass
class StateAction:
    kind: str  # 'entry', 'do', 'exit'
    name: str
    body: str | None = None


@dataclass
class State:
    name: str
    doc: str | None = None
    entry_actions: list[StateAction] = field(default_factory=list)
    do_actions: list[StateAction] = field(default_factory=list)
    exit_actions: list[StateAction] = field(default_factory=list)


@dataclass
class Transition:
    from_state: str
    to_state: str
    condition: str | None = None
    doc: str | None = None


@dataclass
class StateMachine:
    states: list[State]
    transitions: list[Transition]


@dataclass
class EnumDef:
    name: str
    values: list[str]


@dataclass
class PartDef:
    name: str
    attributes: list[Attribute]
    ports: list[Port]
    state_machine: StateMachine | None = None


@dataclass
class Package:
    name: str
    enums: dict[str, EnumDef]
    port_defs: dict[str, Port]
    parts: dict[str, PartDef]


def parse_sysml(content: str, filename: str) -> Package:
    """Parse SysML 2 textual syntax"""

    # Extract package name
    pkg_match = re.search(r'package\s+(\w+)\s*\{', content)
    pkg_name = pkg_match.group(1) if pkg_match else Path(filename).stem

    enums = {}
    port_defs = {}
    parts = {}

    # Parse enum definitions
    for match in re.finditer(r'enum def (\w+)\s*\{([^}]+)\}', content, re.DOTALL):
        enum_name = match.group(1)
        enum_body = match.group(2)
        values = [
            v.strip().rstrip(';') for v in enum_body.strip().split('\n') if v.strip() and not v.strip().startswith('//')
        ]
        enums[enum_name] = EnumDef(enum_name, values)

    # Parse port definitions
    for match in re.finditer(r'port def (\w+)\s*\{([^}]+)\}', content, re.DOTALL):
        port_name = match.group(1)
        port_body = match.group(2)

        signals = []
        attributes = []

        # Parse signals in port
        for sig in re.finditer(r'(in|out)\s+signal\s+(\w+)\s*:\s*(\w+)(?:\[(\d+)\])?;', port_body):
            direction = sig.group(1)
            sig_name = sig.group(2)
            sig_type = sig.group(3)
            array_size = int(sig.group(4)) if sig.group(4) else None
            signals.append(Signal(sig_name, sig_type, direction, array_size))

        # Parse attributes in port
        for attr in re.finditer(r'attribute\s+(\w+)\s*:\s*(\w+)(?:\s*=\s*([^;]+))?;', port_body):
            attr_name = attr.group(1)
            attr_type = attr.group(2)
            default = attr.group(3).strip() if attr.group(3) else None
            attributes.append(Attribute(attr_name, attr_type, default))

        port_defs[port_name] = Port(port_name, port_name, signals, False, attributes)

    # Parse part definitions (with nested braces support)
    def extract_balanced_braces(text, start_pos):
        """Extract content within balanced braces starting at start_pos"""
        level = 0
        i = start_pos
        start = -1
        while i < len(text):
            if text[i] == '{':
                if level == 0:
                    start = i + 1
                level += 1
            elif text[i] == '}':
                level -= 1
                if level == 0:
                    return text[start:i], i
            i += 1
        return None, -1

    part_pattern = re.compile(r'part def (\w+)\s*\{')
    for match in part_pattern.finditer(content):
        part_name = match.group(1)
        body_content, end_pos = extract_balanced_braces(content, match.end() - 1)
        if body_content is None:
            continue
        part_body = body_content

        attributes = []
        ports = []
        state_machine = None

        # Parse attributes
        for attr in re.finditer(
            r'attribute\s+(?:(\w+(?:\s*,\s*\w+)*)\s*:\s*(\w+)|(\w+)\s*:\s*(\w+))(?:\s*=\s*([^;]+))?;', part_body
        ):
            if attr.group(1):  # Multiple attributes with same type
                attr_names = [n.strip() for n in attr.group(1).split(',')]
                attr_type = attr.group(2)
                default = attr.group(5).strip() if attr.group(5) else None
                for attr_name in attr_names:
                    attributes.append(Attribute(attr_name, attr_type, default))
            else:  # Single attribute
                attr_name = attr.group(3)
                attr_type = attr.group(4)
                default = attr.group(5).strip() if attr.group(5) else None
                attributes.append(Attribute(attr_name, attr_type, default))

        # Parse ports
        for port in re.finditer(r'port\s+(\w+)\s*:\s*(~?)(\w+);', part_body):
            port_name = port.group(1)
            conjugated = port.group(2) == '~'
            port_type = port.group(3)

            if port_type in port_defs:
                port_def = port_defs[port_type]
                ports.append(
                    Port(port_name, port_type, port_def.signals.copy(), conjugated, port_def.attributes.copy())
                )

        # Parse state machine
        sm_pattern = re.search(r'state machine\s*\{', part_body)
        if sm_pattern:
            sm_body, _ = extract_balanced_braces(part_body, sm_pattern.end() - 1)
            if sm_body:
                states = []
                transitions = []

                # Parse transitions first (handle optional when clause and doc)
                for t in re.finditer(
                    r'transition\s+(\w+)\s+to\s+(\w+)(?:\s+when\s+([^\n;]+))?(?:\s+doc\s+"([^"]+)")?;',
                    sm_body,
                    re.MULTILINE,
                ):
                    from_state = t.group(1)
                    to_state = t.group(2)
                    condition = t.group(3).strip() if t.group(3) else None
                    doc = t.group(4) if t.group(4) else None
                    transitions.append(Transition(from_state, to_state, condition, doc))

                # Remove transition lines from sm_body before parsing states
                sm_body_no_transitions = re.sub(r'transition\s+\w+\s+to\s+\w+[^;]*;', '', sm_body)

                # Parse states with optional doc blocks and actions
                for s in re.finditer(r'state\s+(\w+)\s*\{', sm_body_no_transitions):
                    state_name = s.group(1)
                    doc = None
                    entry_actions = []
                    do_actions = []
                    exit_actions = []

                    # Extract balanced braces for state body
                    state_body, _ = extract_balanced_braces(sm_body_no_transitions, s.end() - 1)
                    if state_body:
                        doc_match = re.search(r'doc\s+"([^"]+)"', state_body)
                        if doc_match:
                            doc = doc_match.group(1)

                        # Parse entry actions
                        for action in re.finditer(r'entry\s+action\s+(\w+)\s*\{([^}]+)\}', state_body, re.DOTALL):
                            action_name = action.group(1)
                            action_body = action.group(2).strip() if action.group(2) else None
                            entry_actions.append(StateAction('entry', action_name, action_body))

                        # Parse do actions
                        for action in re.finditer(r'do\s+action\s+(\w+)\s*\{([^}]+)\}', state_body, re.DOTALL):
                            action_name = action.group(1)
                            action_body = action.group(2).strip() if action.group(2) else None
                            do_actions.append(StateAction('do', action_name, action_body))

                        # Parse exit actions
                        for action in re.finditer(r'exit\s+action\s+(\w+)\s*\{([^}]+)\}', state_body, re.DOTALL):
                            action_name = action.group(1)
                            action_body = action.group(2).strip() if action.group(2) else None
                            exit_actions.append(StateAction('exit', action_name, action_body))

                    states.append(State(state_name, doc, entry_actions, do_actions, exit_actions))

                if states:
                    state_machine = StateMachine(states, transitions)

        parts[part_name] = PartDef(part_name, attributes, ports, state_machine)

    return Package(pkg_name, enums, port_defs, parts)


def rust_type(sysml_type: str) -> str:
    """Convert SysML types to Rust types"""
    mapping = {'Real': 'f32', 'Integer': 'i32', 'Boolean': 'bool', 'String': '&str'}
    return mapping.get(sysml_type, sysml_type)


def rust_default(sysml_type: str, default_value: str | None) -> str:
    """Get Rust default value"""
    if default_value:
        if sysml_type == 'Real':
            return f'{default_value}.0' if '.' not in default_value else default_value
        elif sysml_type == 'Boolean':
            return default_value.lower()
        else:
            return default_value

    defaults = {'Real': '0.0', 'Integer': '0', 'Boolean': 'false', 'String': '""'}
    return defaults.get(sysml_type, 'Default::default()')


def parse_assignment(action_body: str) -> list[tuple[str, str]]:
    """Parse SysML assignments from action body.

    Returns list of (variable, expression) tuples.
    Example: "cycle_count := 0" -> [("cycle_count", "0")]
    """
    assignments = []
    if not action_body:
        return assignments

    # Remove comments
    body_no_comments = re.sub(r'//.*$', '', action_body, flags=re.MULTILINE)

    # Match patterns like: variable := expression
    for match in re.finditer(r'(\w+)\s*:=\s*([^;]+)', body_no_comments):
        var_name = match.group(1).strip()
        expression = match.group(2).strip()
        # Skip if expression is empty or looks like a comment
        if expression and not expression.startswith('//'):
            assignments.append((var_name, expression))

    return assignments


def sysml_expr_to_rust(expr: str, attributes: list[Attribute]) -> str:
    """Convert SysML expression to Rust expression.

    Handles:
    - Literals: 0, 1.0, true, false
    - Variables: cycle_count → self.cycle_count
    - Binary ops: cycle_count + 1 → self.cycle_count + 1
    - Unary ops: not led_state → !self.led_state
    """
    expr = expr.strip()

    # Boolean literals
    if expr.lower() == 'true':
        return 'true'
    if expr.lower() == 'false':
        return 'false'

    # Numeric literals with decimal point
    if re.match(r'^-?\d+\.\d+$', expr):
        return expr

    # Integer literals
    if re.match(r'^-?\d+$', expr):
        return expr

    # Handle 'not' operator - apply before variable replacement
    if expr.lower().startswith('not '):
        inner = expr[4:].strip()
        # Replace attribute references in the inner expression
        for attr in attributes:
            inner = re.sub(rf'\b{attr.name}\b', f'self.{attr.name}', inner)
        return f'!{inner}'

    # Replace SysML operators with Rust equivalents
    expr = expr.replace(' and ', ' && ')
    expr = expr.replace(' or ', ' || ')

    # Replace attribute references with self.attribute
    for attr in attributes:
        expr = re.sub(rf'\b{attr.name}\b', f'self.{attr.name}', expr)

    return expr


def sysml_condition_to_rust(condition: str, attributes: list[Attribute]) -> str:
    """Convert SysML condition to Rust boolean expression.

    Handles:
    - Comparisons: cycle_count > 100 → self.cycle_count > 100
    - Boolean variables: enabled → self.enabled
    - Logical operators: enabled && cycle_count > 100
    """
    condition = condition.strip()

    # Replace SysML operators with Rust equivalents
    condition = condition.replace(' and ', ' && ')
    condition = condition.replace(' or ', ' || ')
    condition = condition.replace(' not ', ' !')

    # Replace attribute references with self.attribute
    for attr in attributes:
        condition = re.sub(rf'\b{attr.name}\b', f'self.{attr.name}', condition)

    return condition


def generate_rust_code(package: Package, output_dir: Path):
    """Generate pure Rust data structures from SysML"""

    output_dir.mkdir(parents=True, exist_ok=True)
    output_file = output_dir / f'{package.name.lower()}.rs'

    content = []
    content.append('// Auto-generated from SysML 2 model')
    content.append(f'// Package: {package.name}')
    content.append('')

    # Generate enums
    if package.enums:
        content.append('// Enumeration definitions')
        content.append('')
        for enum_name, enum_def in package.enums.items():
            content.append('#[derive(Debug, Clone, Copy, PartialEq, Eq)]')
            content.append(f'pub enum {enum_name} {{')
            for value in enum_def.values:
                variant = to_pascal_case(value)
                content.append(f'    {variant},')
            content.append('}')
            content.append('')

    # Generate port traits
    if package.port_defs:
        content.append('// Port trait definitions')
        content.append('')
        for port_name, port_def in package.port_defs.items():
            content.append(f'pub trait {port_name} {{')

            for sig in port_def.signals:
                rust_sig_type = rust_type(sig.type)
                if sig.array_size:
                    rust_sig_type = f'[{rust_sig_type}; {sig.array_size}]'

                if sig.direction == 'out':
                    # Output signals: both get and set
                    content.append(f'    fn get_{sig.name}(&self) -> {rust_sig_type};')
                    content.append(f'    fn set_{sig.name}(&mut self, value: {rust_sig_type});')
                else:  # 'in'
                    # Input signals: only get (read-only)
                    content.append(f'    fn get_{sig.name}(&self) -> {rust_sig_type};')

            content.append('}')
            content.append('')

    # Generate parts
    for part_name, part in package.parts.items():
        content.append(f'// {part_name}')
        content.append('')

        # Generate state enum if part has state machine
        if part.state_machine:
            sm = part.state_machine
            content.append('#[derive(Debug, Clone, Copy, PartialEq, Eq)]')
            content.append(f'pub enum {part_name}State {{')
            for state in sm.states:
                state_variant = to_pascal_case(state.name)
                if state.doc:
                    content.append(f'    /// {state.doc}')
                content.append(f'    {state_variant},')
            content.append('}')
            content.append('')

        # Generate struct
        content.append(f'pub struct {part_name} {{')

        # Add state field if state machine exists
        if part.state_machine:
            content.append(f'    pub state: {part_name}State,')

        # Add attributes
        for attr in part.attributes:
            content.append(f'    pub {attr.name}: {rust_type(attr.type)},')

        # Add port data storage
        for port in part.ports:
            for sig in port.signals:
                rust_sig_type = rust_type(sig.type)
                if sig.array_size:
                    rust_sig_type = f'[{rust_sig_type}; {sig.array_size}]'
                content.append(f'    pub {port.name}_{sig.name}: {rust_sig_type},')

        content.append('}')
        content.append('')

        # Generate implementation
        content.append(f'impl {part_name} {{')
        content.append('    pub fn new() -> Self {')
        content.append('        Self {')

        if part.state_machine:
            initial_state = to_pascal_case(part.state_machine.states[0].name)
            content.append(f'            state: {part_name}State::{initial_state},')

        for attr in part.attributes:
            default_val = rust_default(attr.type, attr.default)
            content.append(f'            {attr.name}: {default_val},')

        for port in part.ports:
            for sig in port.signals:
                default_val = rust_default(sig.type, None)
                if sig.array_size:
                    default_val = f'[{default_val}; {sig.array_size}]'
                content.append(f'            {port.name}_{sig.name}: {default_val},')

        content.append('        }')
        content.append('    }')
        content.append('')

        # Generate step function if state machine exists
        if part.state_machine:
            sm = part.state_machine
            content.append('    /// Execute one step of the state machine')
            content.append('    pub fn step(&mut self) {')
            content.append('        match self.state {')

            for state in sm.states:
                state_variant = to_pascal_case(state.name)
                content.append(f'            {part_name}State::{state_variant} => {{')

                # Execute do actions (assignments)
                for action in state.do_actions:
                    assignments = parse_assignment(action.body)
                    if assignments:
                        content.append(f'                // Do action: {action.name}')
                        for var_name, expr in assignments:
                            rust_expr = sysml_expr_to_rust(expr, part.attributes)
                            content.append(f'                self.{var_name} = {rust_expr};')

                # Check transitions
                state_transitions = [t for t in sm.transitions if t.from_state == state.name]
                if state_transitions:
                    content.append('')
                    content.append('                // Transitions')
                    for trans in state_transitions:
                        to_variant = to_pascal_case(trans.to_state)
                        if trans.condition:
                            rust_condition = sysml_condition_to_rust(trans.condition, part.attributes)
                            content.append(f'                if {rust_condition} {{')
                            content.append(f'                    self.state = {part_name}State::{to_variant};')
                            content.append('                }')
                        else:
                            # Unconditional transition
                            content.append(f'                self.state = {part_name}State::{to_variant};')

                content.append('            }')

            content.append('        }')
            content.append('    }')

        content.append('}')
        content.append('')

        # Implement port traits
        for port in part.ports:
            if port.type in package.port_defs:
                content.append(f'impl {port.type} for {part_name} {{')

                for sig in port.signals:
                    rust_sig_type = rust_type(sig.type)
                    if sig.array_size:
                        rust_sig_type = f'[{rust_sig_type}; {sig.array_size}]'

                    if sig.direction == 'out':
                        # Output signals: both get and set
                        content.append(f'    fn get_{sig.name}(&self) -> {rust_sig_type} {{')
                        content.append(f'        self.{port.name}_{sig.name}')
                        content.append('    }')
                        content.append('')
                        content.append(f'    fn set_{sig.name}(&mut self, value: {rust_sig_type}) {{')
                        content.append(f'        self.{port.name}_{sig.name} = value;')
                        content.append('    }')
                    else:  # 'in'
                        # Input signals: only get (read-only)
                        content.append(f'    fn get_{sig.name}(&self) -> {rust_sig_type} {{')
                        content.append(f'        self.{port.name}_{sig.name}')
                        content.append('    }')

                content.append('}')
                content.append('')

    # Write output file
    with open(output_file, 'w') as f:
        f.write('\n'.join(content))

    print(f'Generated: {output_file}')


def main():
    """Main code generation entry point"""
    import argparse

    parser = argparse.ArgumentParser(description='Generate Rust code from SysML 2 models')
    parser.add_argument('input', nargs='+', help='SysML 2 model files (.sysml)')
    parser.add_argument('--output-dir', default='generated', help='Output directory for generated code')

    args = parser.parse_args()

    output_path = Path(args.output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    # Process all input files
    for sysml_file in args.input:
        print(f'\nProcessing: {sysml_file}')

        with open(sysml_file) as f:
            content = f.read()

        package = parse_sysml(content, sysml_file)
        generate_rust_code(package, output_path)

    print('\n✓ Code generation complete!')
    print(f'  Output directory: {output_path}')


if __name__ == '__main__':
    main()
