//! Integration and end-to-end tests for sysml_codegen.
//!
//! Unit tests live inline in their respective modules (mcrl2_expr, parser).
//! This file covers cross-module integration tests and e2e codegen tests.

use crate::codegen;
use crate::mcrl2_compose;
use crate::mcrl2_render;
use crate::parser;
use std::collections::HashMap;

fn model_path(name: &str) -> String {
    format!("{}/{}", env!("MODEL_DIR"), name)
}

/// Parse a single model file by name.
fn parse_model(name: &str) -> crate::ast::Package {
    let path = model_path(name);
    let content = std::fs::read_to_string(&path).expect("read model file");
    parser::parse_sysml(&content, &path)
}

// ============================================================================
// Integration Tests: Per-Actor mCRL2 Rendering
// ============================================================================

#[test]
fn test_button_mcrl2_sort() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    assert_eq!(results.len(), 1);
    let (name, content, _) = &results[0];
    assert_eq!(name, "ButtonActor");
    assert!(
        content.contains(
            "sort State = struct Idle | Debouncing | PressedState | Notifying | Released;"
        ),
        "missing state sort declaration"
    );
}

#[test]
fn test_button_mcrl2_constants() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    let (_, content, _) = &results[0];
    assert!(
        content.contains("map debounce_threshold: Nat;"),
        "missing constant map"
    );
    assert!(
        content.contains("eqn debounce_threshold = 5;"),
        "missing constant eqn"
    );
}

#[test]
fn test_button_mcrl2_process() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    let (_, content, _) = &results[0];
    assert!(
        content.contains(
            "proc ButtonActor(s: State, pressed: Bool, press_count: Nat, debounce_counter: Nat)"
        ),
        "missing process declaration:\n{content}"
    );
}

#[test]
fn test_button_mcrl2_init() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    let (_, content, _) = &results[0];
    assert!(
        content.contains("init ButtonActor(Idle, false, 0, 0);"),
        "missing init:\n{content}"
    );
}

#[test]
fn test_button_mcrl2_guard() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    let (_, content, _) = &results[0];
    assert!(
        content.contains("step(Debouncing, PressedState)"),
        "missing Debouncing->PressedState step"
    );
}

#[test]
fn test_button_mcrl2_self_loop() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    let (_, content, _) = &results[0];
    assert!(
        content.contains("step(Debouncing, Debouncing)"),
        "missing self-loop for Debouncing:\n{content}"
    );
}

#[test]
fn test_button_mcrl2_env_input() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    let (_, content, _) = &results[0];
    assert!(
        content.contains("sum pressed_v: Bool . env_pressed(pressed_v)"),
        "missing env input action:\n{content}"
    );
}

#[test]
fn test_button_mcrl2_deadlock_freedom() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    let (_, _, props) = &results[0];
    let deadlock = props
        .iter()
        .find(|(name, _)| name.contains("deadlock_freedom"));
    assert!(deadlock.is_some(), "missing deadlock freedom property");
    let (_, mcf) = deadlock.expect("deadlock");
    assert!(mcf.contains("[true*]<true>true"), "wrong deadlock formula");
}

#[test]
fn test_button_mcrl2_liveness() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    let (_, _, props) = &results[0];
    let liveness = props.iter().find(|(name, _)| name.contains("liveness"));
    assert!(liveness.is_some(), "missing liveness property");
    let (_, mcf) = liveness.expect("liveness");
    assert!(mcf.contains("nu X. mu Y."), "wrong liveness formula");
}

#[test]
fn test_control_mcrl2_conditional() {
    let pkg = parse_model("control.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    assert_eq!(results.len(), 1);
    let (_, content, _) = &results[0];
    assert!(
        content.contains("step(Running,"),
        "missing Running transitions:\n{content}"
    );
}

#[test]
fn test_maintenance_mcrl2_if_else() {
    let pkg = parse_model("maintenance.sysml");
    let results = mcrl2_render::render_mcrl2(&pkg);
    assert_eq!(results.len(), 1);
    let (_, content, _) = &results[0];
    assert!(
        content.contains("step(Checking, Toggling)"),
        "missing Checking->Toggling"
    );
    assert!(
        content.contains("step(Checking, Reporting)"),
        "missing Checking->Reporting fallback"
    );
}

// ============================================================================
// Integration Tests: Composed mCRL2 System
// ============================================================================

/// Helper: parse all models, resolve parts, and render composed mCRL2 for PicoSystem.
fn render_composed_mcrl2() -> (String, Vec<(String, String)>) {
    let button_pkg = parse_model("button.sysml");
    let control_pkg = parse_model("control.sysml");
    let maintenance_pkg = parse_model("maintenance.sysml");
    let system_pkg = parse_model("system.sysml");

    let all_packages = vec![&button_pkg, &control_pkg, &maintenance_pkg, &system_pkg];
    let all_part_defs: HashMap<String, &crate::ast::PartDef> = all_packages
        .iter()
        .flat_map(|p| p.parts.iter())
        .map(|pd| (pd.name.clone(), pd))
        .collect();

    let all_port_defs: Vec<&crate::ast::Port> = all_packages
        .iter()
        .flat_map(|p| p.port_defs.iter())
        .collect();

    let system_part = system_pkg
        .parts
        .iter()
        .find(|p| !p.part_instances.is_empty())
        .expect("system part");

    let mut resolved: HashMap<String, &crate::ast::PartDef> = HashMap::new();
    for inst in &system_part.part_instances {
        if let Some(pd) = all_part_defs.get(&inst.typ) {
            resolved.insert(inst.name.clone(), pd);
        }
    }

    mcrl2_compose::render_composed_mcrl2(system_part, &resolved, &all_port_defs)
}

#[test]
fn test_composed_mcrl2_state_sorts() {
    let (content, _) = render_composed_mcrl2();
    assert!(content.contains("ButtonState"), "missing ButtonState sort");
    assert!(
        content.contains("ControlState"),
        "missing ControlState sort"
    );
    assert!(
        content.contains("MaintenanceState"),
        "missing MaintenanceState sort"
    );
}

#[test]
fn test_composed_mcrl2_message_sorts() {
    let (content, _) = render_composed_mcrl2();
    assert!(content.contains("ButtonMsg"), "missing ButtonMsg sort");
    assert!(
        content.contains("MaintenanceMsg"),
        "missing MaintenanceMsg sort"
    );
}

#[test]
fn test_composed_mcrl2_buffer_processes() {
    let (content, _) = render_composed_mcrl2();
    assert!(
        content.contains("Buffer_button_to_control"),
        "missing buffer process"
    );
    assert!(
        content.contains("#q < 4"),
        "missing capacity guard for button->control"
    );
}

#[test]
fn test_composed_mcrl2_comm_operator() {
    let (content, _) = render_composed_mcrl2();
    assert!(content.contains("comm({"), "missing comm operator");
    assert!(content.contains("allow({"), "missing allow operator");
}

#[test]
fn test_composed_mcrl2_init_parallel() {
    let (content, _) = render_composed_mcrl2();
    assert!(content.contains("||"), "missing parallel composition");
}

#[test]
fn test_composed_mcrl2_deadlock_freedom() {
    let (_, props) = render_composed_mcrl2();
    let deadlock = props
        .iter()
        .find(|(name, _)| name.contains("deadlock_freedom"));
    assert!(deadlock.is_some(), "missing composed deadlock freedom");
    let (_, mcf) = deadlock.expect("deadlock");
    assert!(mcf.contains("[true*]<true>true"));
}

#[test]
fn test_composed_mcrl2_liveness() {
    let (_, props) = render_composed_mcrl2();
    let liveness = props.iter().find(|(name, _)| name.contains("liveness"));
    assert!(liveness.is_some(), "missing composed liveness");
    let (_, mcf) = liveness.expect("liveness");
    assert!(
        mcf.contains("nu X. mu Y."),
        "missing liveness formula structure"
    );
}

// ============================================================================
// End-to-End Codegen Tests
// ============================================================================

#[test]
fn test_e2e_rust_codegen() {
    let pkg = parse_model("button.sysml");
    let rust_code = codegen::generate(&pkg);
    assert!(!rust_code.is_empty(), "empty rust output");
    assert!(
        rust_code.contains("struct"),
        "missing struct in rust output"
    );
    assert!(rust_code.contains("impl"), "missing impl in rust output");
}

#[test]
fn test_e2e_mcrl2_codegen() {
    let pkg = parse_model("button.sysml");
    let results = codegen::generate_mcrl2(&pkg);
    assert!(!results.is_empty(), "empty mCRL2 results");
    let (part_name, mcrl2, props) = &results[0];
    assert_eq!(part_name, "ButtonActor");
    assert!(!mcrl2.is_empty(), "empty mCRL2 content");
    assert!(
        props.len() >= 2,
        "should have deadlock + liveness properties"
    );
}

#[test]
fn test_e2e_rust_codegen_all_models() {
    for model in &["button.sysml", "control.sysml", "maintenance.sysml"] {
        let pkg = parse_model(model);
        let rust_code = codegen::generate(&pkg);
        assert!(!rust_code.is_empty(), "empty rust output for {model}");
    }
}

#[test]
fn test_e2e_mcrl2_codegen_all_models() {
    for model in &["button.sysml", "control.sysml", "maintenance.sysml"] {
        let pkg = parse_model(model);
        let results = codegen::generate_mcrl2(&pkg);
        assert!(!results.is_empty(), "empty mCRL2 results for {model}");
    }
}

// ============================================================================
// Integration Tests: Timed mCRL2 Rendering
// ============================================================================

#[test]
fn test_extract_timing_control() {
    let pkg = parse_model("control.sysml");
    let part = pkg
        .parts
        .iter()
        .find(|p| p.name == "RealtimeControlActor")
        .expect("control part");
    let timing = mcrl2_render::extract_timing_info(part);
    assert_eq!(timing.execution_period_ms, Some(1));
    assert_eq!(timing.max_execution_time_us, Some(800));
    assert_eq!(timing.max_jitter_us, Some(50));
    assert_eq!(timing.debounce_period_ms, None);
}

#[test]
fn test_compute_time_step_control() {
    let pkg = parse_model("control.sysml");
    let part = pkg
        .parts
        .iter()
        .find(|p| p.name == "RealtimeControlActor")
        .expect("control part");
    let timing = mcrl2_render::extract_timing_info(part);
    let step = mcrl2_render::compute_time_step(&timing);
    // GCD(1000, 800, 50) = 50
    assert_eq!(step, 50, "time step for control should be 50us");
}

#[test]
fn test_control_timed_structure() {
    let pkg = parse_model("control.sysml");
    let results = mcrl2_render::render_timed_mcrl2(&pkg);
    assert_eq!(results.len(), 1);
    let (name, content, _) = &results[0];
    assert_eq!(name, "RealtimeControlActor_timed");
    assert!(
        content.contains("elapsed: Nat"),
        "missing elapsed parameter:\n{content}"
    );
    assert!(
        content.contains("activate"),
        "missing activate action:\n{content}"
    );
    assert!(content.contains("tick"), "missing tick action:\n{content}");
    assert!(
        content.contains("PERIOD_TICKS"),
        "missing PERIOD_TICKS constant:\n{content}"
    );
}

#[test]
fn test_control_timed_deadlock_freedom_prop() {
    let pkg = parse_model("control.sysml");
    let results = mcrl2_render::render_timed_mcrl2(&pkg);
    let (_, _, props) = &results[0];
    let deadlock = props
        .iter()
        .find(|(name, _)| name.contains("timed_deadlock_freedom"));
    assert!(
        deadlock.is_some(),
        "missing timed deadlock freedom property"
    );
    let (_, mcf) = deadlock.expect("deadlock");
    assert!(
        mcf.contains("[true*]<true>true"),
        "deadlock freedom property should verify all states have successors"
    );
}

#[test]
fn test_button_timed_debounce() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_timed_mcrl2(&pkg);
    assert_eq!(results.len(), 1);
    let (name, content, _) = &results[0];
    assert_eq!(name, "ButtonActor_timed");
    assert!(
        content.contains("DEBOUNCE_TICKS"),
        "missing DEBOUNCE_TICKS:\n{content}"
    );
    assert!(
        content.contains("elapsed >= DEBOUNCE_TICKS"),
        "missing debounce guard on env input:\n{content}"
    );
    // With WCET annotations, the phase model is used; event-driven actors (no period)
    // don't get deadline_miss since there's no periodic activation to miss
    assert!(
        content.contains("phase"),
        "should have phase parameter with WCET:\n{content}"
    );
    assert!(
        !content.contains("deadline_miss"),
        "event-driven actor should not have deadline_miss:\n{content}"
    );
}

#[test]
fn test_maintenance_timed() {
    let pkg = parse_model("maintenance.sysml");
    let results = mcrl2_render::render_timed_mcrl2(&pkg);
    assert_eq!(results.len(), 1);
    let (name, content, _) = &results[0];
    assert_eq!(name, "MaintenanceActor_timed");
    assert!(
        content.contains("PERIOD_TICKS"),
        "missing PERIOD_TICKS:\n{content}"
    );
    // Maintenance has no jitter attribute
    assert!(
        !content.contains("JITTER_TICKS"),
        "should not have JITTER_TICKS:\n{content}"
    );
}

#[test]
fn test_e2e_timed_all_models() {
    for model in &["button.sysml", "control.sysml", "maintenance.sysml"] {
        let pkg = parse_model(model);
        let results = codegen::generate_timed_mcrl2(&pkg);
        assert!(!results.is_empty(), "expected timed spec for {model}");
        let (_, _, props) = &results[0];
        assert!(
            props.len() >= 2,
            "expected >= 2 timed properties for {model}, got {}",
            props.len()
        );
    }
}

#[test]
fn test_e2e_system_channels_codegen() {
    let button_pkg = parse_model("button.sysml");
    let control_pkg = parse_model("control.sysml");
    let maintenance_pkg = parse_model("maintenance.sysml");
    let system_pkg = parse_model("system.sysml");

    let all_packages = vec![&button_pkg, &control_pkg, &maintenance_pkg, &system_pkg];
    let all_part_defs: HashMap<String, &crate::ast::PartDef> = all_packages
        .iter()
        .flat_map(|p| p.parts.iter())
        .map(|pd| (pd.name.clone(), pd))
        .collect();

    let system_part = system_pkg
        .parts
        .iter()
        .find(|p| !p.part_instances.is_empty())
        .expect("system part");

    let mut resolved: HashMap<String, &crate::ast::PartDef> = HashMap::new();
    for inst in &system_part.part_instances {
        if let Some(pd) = all_part_defs.get(&inst.typ) {
            resolved.insert(inst.name.clone(), pd);
        }
    }

    let channels_code = codegen::generate_system_channels(system_part, &resolved);
    assert!(
        channels_code.contains("use crate::messages::{ButtonMessage, MaintenanceMessage};"),
        "missing message-type import:\n{channels_code}"
    );
    assert!(
        channels_code.contains(
            "pub static BUTTON_TO_CONTROL: Channel<CriticalSectionRawMutex, ButtonMessage, 4>"
        ),
        "missing BUTTON_TO_CONTROL static:\n{channels_code}"
    );
    assert!(
        channels_code.contains(
            "pub static MAINTENANCE_TO_CONTROL: Channel<CriticalSectionRawMutex, MaintenanceMessage, 2>"
        ),
        "missing MAINTENANCE_TO_CONTROL static:\n{channels_code}"
    );
    assert!(
        !channels_code.contains("define_channels!"),
        "should not invoke the helper macro any more:\n{channels_code}"
    );
}

// ============================================================================
// Integration Tests: WCET Rust Constants
// ============================================================================

#[test]
fn test_wcet_rust_constants_control() {
    let pkg = parse_model("control.sysml");
    let rust_code = codegen::generate(&pkg);
    // Initializing: initialize(200) + count(10) = 210
    assert!(
        rust_code.contains("pub const WCET_INITIALIZING_US: u64 = 210;"),
        "missing WCET_INITIALIZING_US constant:\n{rust_code}"
    );
    // Running: count(10) = 10
    assert!(
        rust_code.contains("pub const WCET_RUNNING_US: u64 = 10;"),
        "missing WCET_RUNNING_US constant:\n{rust_code}"
    );
    // Processing: no WCET = 0
    assert!(
        rust_code.contains("pub const WCET_PROCESSING_US: u64 = 0;"),
        "missing WCET_PROCESSING_US constant:\n{rust_code}"
    );
    // Error: set_safe_state(20) = 20
    assert!(
        rust_code.contains("pub const WCET_ERROR_US: u64 = 20;"),
        "missing WCET_ERROR_US constant:\n{rust_code}"
    );
    // Max across all states = 210
    assert!(
        rust_code.contains("pub const WCET_MAX_US: u64 = 210;"),
        "missing WCET_MAX_US constant:\n{rust_code}"
    );
}

// ============================================================================
// Integration Tests: WCET Annotations and Phase-Based Timed Model
// ============================================================================

#[test]
fn test_parse_wcet_action_def() {
    let pkg = parse_model("control.sysml");
    let init_def = pkg
        .action_defs
        .iter()
        .find(|a| a.name == "initialize")
        .expect("initialize action def");
    assert_eq!(
        init_def.wcet_us,
        Some(200),
        "initialize should have wcet 200"
    );
    // Body should not contain the wcet line
    assert!(
        !init_def.body.contains("wcet"),
        "wcet should be stripped from body"
    );
    assert!(
        init_def.body.contains("cycle_count := 0"),
        "body should still contain assignments"
    );

    let count_def = pkg
        .action_defs
        .iter()
        .find(|a| a.name == "count")
        .expect("count action def");
    assert_eq!(count_def.wcet_us, Some(10), "count should have wcet 10");
}

#[test]
fn test_parse_wcet_propagation() {
    let pkg = parse_model("button.sysml");
    let part = &pkg.parts[0];
    let sm = part.state_machine.as_ref().expect("has sm");

    let idle = sm
        .states
        .iter()
        .find(|s| s.name == "Idle")
        .expect("Idle state");
    assert_eq!(idle.entry_actions.len(), 1);
    assert_eq!(
        idle.entry_actions[0].wcet_us,
        Some(10),
        "reset_debounce wcet should propagate"
    );

    let debouncing = sm
        .states
        .iter()
        .find(|s| s.name == "Debouncing")
        .expect("Debouncing state");
    assert_eq!(debouncing.do_actions.len(), 1);
    assert_eq!(
        debouncing.do_actions[0].wcet_us,
        Some(5),
        "increment_debounce wcet should propagate"
    );
}

#[test]
fn test_wcet_state_phase_control() {
    let pkg = parse_model("control.sysml");
    let part = pkg
        .parts
        .iter()
        .find(|p| p.name == "RealtimeControlActor")
        .expect("control part");
    let _sm = part.state_machine.as_ref().expect("has sm");

    // With WCET values included, GCD changes. Let's check the phase values.
    // Control: initialize(200) + count(10) in Initializing = 210us
    //          count(10) in Running = 10us
    //          set_safe_state(20) in Error = 20us
    // Time step: GCD(1000, 800, 50, 200, 10, 20) = 10
    // Initializing phase: ceil(210 / 10) = 21 ticks
    // Running phase: ceil(10 / 10) = 1 tick
    let results = mcrl2_render::render_timed_mcrl2(&pkg);
    let (_, content, _) = &results[0];
    assert!(
        content.contains("PHASE_Initializing"),
        "missing PHASE_Initializing"
    );
    assert!(content.contains("PHASE_Running"), "missing PHASE_Running");
    // Verify the content has phase parameter
    assert!(content.contains("phase: Nat"), "missing phase parameter");
}

#[test]
fn test_control_wcet_timed_structure() {
    let pkg = parse_model("control.sysml");
    let results = mcrl2_render::render_timed_mcrl2(&pkg);
    let (_, content, _) = &results[0];

    // Should contain WCET phase model elements
    assert!(
        content.contains("phase"),
        "missing phase in timed model:\n{content}"
    );
    assert!(
        content.contains("PHASE_"),
        "missing PHASE_ constants:\n{content}"
    );
    assert!(
        content.contains("deadline_miss"),
        "missing deadline_miss action:\n{content}"
    );
    // Phase guards on step transitions
    assert!(
        content.contains("phase == 0 && s =="),
        "missing phase guard on step:\n{content}"
    );
    // Computation tick
    assert!(
        content.contains("phase > 0"),
        "missing computation tick:\n{content}"
    );
    // Idle tick
    assert!(
        content.contains("phase == 0 && elapsed < MAX_ELAPSED"),
        "missing idle tick:\n{content}"
    );
}

#[test]
fn test_control_wcet_deadline_prop() {
    let pkg = parse_model("control.sysml");
    let results = mcrl2_render::render_timed_mcrl2(&pkg);
    let (_, _, props) = &results[0];

    let deadline = props
        .iter()
        .find(|(name, _)| name.contains("timed_deadline"));
    assert!(deadline.is_some(), "missing timed_deadline property");
    let (_, mcf) = deadline.expect("deadline");
    assert!(
        mcf.contains("[true*][deadline_miss]false"),
        "wrong deadline formula:\n{mcf}"
    );
}

#[test]
fn test_button_wcet_timed() {
    let pkg = parse_model("button.sysml");
    let results = mcrl2_render::render_timed_mcrl2(&pkg);
    let (_, content, _) = &results[0];

    // Button is event-driven (debounce), should have phase model
    assert!(content.contains("phase: Nat"), "missing phase parameter");
    assert!(content.contains("PHASE_Idle"), "missing PHASE_Idle");
    assert!(
        content.contains("PHASE_Debouncing"),
        "missing PHASE_Debouncing"
    );
    assert!(
        content.contains("PHASE_PressedState"),
        "missing PHASE_PressedState"
    );
    // Event-driven: no deadline_miss (no periodic activation to miss)
    assert!(
        !content.contains("deadline_miss"),
        "event-driven should not have deadline_miss"
    );
}

#[test]
fn test_e2e_wcet_all_models() {
    for (model, expect_timed, min_props) in &[
        ("button.sysml", true, 2), // deadlock + response (event-driven, no deadline)
        ("control.sysml", true, 3), // deadlock + deadline + response
        ("maintenance.sysml", true, 3), // deadlock + deadline + response
    ] {
        let pkg = parse_model(model);
        let results = codegen::generate_timed_mcrl2(&pkg);
        if *expect_timed {
            assert!(!results.is_empty(), "expected timed spec for {model}");
            let (_, content, props) = &results[0];
            assert!(
                props.len() >= *min_props,
                "expected >= {min_props} timed properties for {model}, got {}: {:?}",
                props.len(),
                props.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
            );
            // All WCET models should have phase
            assert!(
                content.contains("phase"),
                "expected phase model for {model}"
            );
        } else {
            assert!(results.is_empty(), "expected no timed spec for {model}");
        }
    }
}

// ============================================================================
// Integration Tests: wcet_for_state() Method Generation
// ============================================================================

#[test]
fn test_wcet_for_state_method_control() {
    let pkg = parse_model("control.sysml");
    let rust_code = codegen::generate(&pkg);
    assert!(
        rust_code.contains("pub const fn wcet_for_state(state: RealtimeControlActorState) -> u64"),
        "missing wcet_for_state method for control:\n{rust_code}"
    );
    // Verify all state arms are present
    assert!(
        rust_code.contains("RealtimeControlActorState::Initializing => Self::WCET_INITIALIZING_US"),
        "missing Initializing arm:\n{rust_code}"
    );
    assert!(
        rust_code.contains("RealtimeControlActorState::Running => Self::WCET_RUNNING_US"),
        "missing Running arm:\n{rust_code}"
    );
    assert!(
        rust_code.contains("RealtimeControlActorState::Error => Self::WCET_ERROR_US"),
        "missing Error arm:\n{rust_code}"
    );
}

#[test]
fn test_wcet_for_state_method_button() {
    let pkg = parse_model("button.sysml");
    let rust_code = codegen::generate(&pkg);
    assert!(
        rust_code.contains("pub const fn wcet_for_state(state: ButtonActorState) -> u64"),
        "missing wcet_for_state method for button:\n{rust_code}"
    );
    assert!(
        rust_code.contains("ButtonActorState::Idle => Self::WCET_IDLE_US"),
        "missing Idle arm:\n{rust_code}"
    );
    assert!(
        rust_code.contains("ButtonActorState::Debouncing => Self::WCET_DEBOUNCING_US"),
        "missing Debouncing arm:\n{rust_code}"
    );
}

// ============================================================================
// Unit Tests: Non-Preemptive Response-Time Analysis
// ============================================================================

#[test]
fn test_rta_control_core0() {
    // Control is the only actor on core 0 (prio=10, C=210)
    // No lower-priority peers → B=0, R=210
    let (response, blocking) = crate::compute_response_time(210, 10, &[(210, 10)]);
    assert_eq!(blocking, 0, "no lower-prio peers on core 0");
    assert_eq!(response, 210, "R = C + 0");
}

#[test]
fn test_rta_button_core1() {
    // Core 1 peers: Button(C=15, prio=8), Maintenance(C=50, prio=5)
    let peers = &[(15, 8), (50, 5)];
    let (response, blocking) = crate::compute_response_time(15, 8, peers);
    assert_eq!(blocking, 50, "max WCET of lower-prio = Maintenance(50)");
    assert_eq!(response, 65, "R = 15 + 50");
}

#[test]
fn test_rta_maintenance_core1() {
    let peers = &[(15, 8), (50, 5)];
    let (response, blocking) = crate::compute_response_time(50, 5, peers);
    assert_eq!(blocking, 0, "no lower-prio peers for maintenance");
    assert_eq!(response, 50, "R = 50 + 0");
}

// ============================================================================
// Integration Tests: SysML Timing Attributes → Rust Constants
// ============================================================================
//
// These constants are what `tasks.rs` and `main_*.rs` consume to keep
// SysML as the single source of truth for actor cadence/priority/core.

#[test]
fn test_timing_constants_control() {
    let pkg = parse_model("control.sysml");
    let rust_code = codegen::generate(&pkg);
    assert!(
        rust_code.contains("pub const EXECUTION_PERIOD_US: u64 = 1_000;"),
        "control should expose 1ms period as 1000us:\n{rust_code}"
    );
    assert!(
        rust_code.contains("pub const MAX_JITTER_US: u64 = 50;"),
        "control should expose max_jitter_us:\n{rust_code}"
    );
    assert!(
        rust_code.contains("pub const MAX_EXECUTION_TIME_US: u64 = 800;"),
        "control should expose deadline:\n{rust_code}"
    );
    assert!(
        rust_code.contains("pub const PRIORITY: u8 = 10;"),
        "control should expose priority:\n{rust_code}"
    );
    assert!(
        rust_code.contains("pub const CORE: u8 = 0;"),
        "control should expose core:\n{rust_code}"
    );
}

#[test]
fn test_timing_constants_maintenance() {
    let pkg = parse_model("maintenance.sysml");
    let rust_code = codegen::generate(&pkg);
    assert!(
        rust_code.contains("pub const EXECUTION_PERIOD_US: u64 = 100_000;"),
        "maintenance should expose 100ms period as 100000us:\n{rust_code}"
    );
    assert!(
        rust_code.contains("pub const PRIORITY: u8 = 5;"),
        "maintenance should expose priority:\n{rust_code}"
    );
}

#[test]
fn test_timing_constants_button_debounce() {
    let pkg = parse_model("button.sysml");
    let rust_code = codegen::generate(&pkg);
    assert!(
        rust_code.contains("pub const DEBOUNCE_PERIOD_US: u64 = 10_000;"),
        "button should expose 10ms debounce as 10000us:\n{rust_code}"
    );
    assert!(
        !rust_code.contains("EXECUTION_PERIOD_US"),
        "event-driven button should not emit EXECUTION_PERIOD_US:\n{rust_code}"
    );
}
