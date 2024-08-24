mod ast;
mod buf;
mod codegen;
mod expr;
mod mcrl2_compose;
mod mcrl2_expr;
mod mcrl2_render;
mod parser;
mod render;
mod rust_ast;
mod translate;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Non-preemptive response-time analysis for a single task.
///
/// In Embassy's cooperative scheduler a high-priority task can be blocked by
/// a running lower-priority task for at most the lower-priority task's full
/// WCET (since there is no preemption mid-execution).
///
/// `wcet` and `priority` are for the task under analysis.
/// `peers` contains `(wcet, priority)` for **all** tasks on the same core
/// (including the task itself — it is filtered out by priority comparison).
///
/// Returns `(response_time, blocking)` where:
///   - `blocking` = max WCET among strictly lower-priority peers
///   - `response_time` = wcet + blocking
#[must_use]
pub fn compute_response_time(wcet: u64, priority: u64, peers: &[(u64, u64)]) -> (u64, u64) {
    let blocking = peers
        .iter()
        .filter(|(_, p)| *p < priority)
        .map(|(c, _)| *c)
        .max()
        .unwrap_or(0);
    (wcet + blocking, blocking)
}

struct ActorTiming {
    name: String,
    core: u64,
    priority: u64,
    max_wcet_us: u64,
    period_us: Option<u64>,
}

/// Non-preemptive response-time schedulability check.
///
/// For each core, compute `R_i` = `C_i` + `B_i` for every actor and verify that
/// periodic actors satisfy `R_i` <= `T_i`. Emits diagnostics to stdout.
fn check_schedulability(packages: &[&ast::Package]) {
    let mut actors: Vec<ActorTiming> = Vec::new();

    for pkg in packages {
        for part in &pkg.parts {
            let Some(sm) = &part.state_machine else {
                continue;
            };

            let core = part
                .attributes
                .iter()
                .find(|a| a.name == "core")
                .and_then(|a| a.default.as_ref())
                .and_then(|d| d.trim().parse::<u64>().ok())
                .unwrap_or(0);

            let priority = part
                .attributes
                .iter()
                .find(|a| a.name == "priority")
                .and_then(|a| a.default.as_ref())
                .and_then(|d| d.trim().parse::<u64>().ok())
                .unwrap_or(0);

            let period_us = part
                .attributes
                .iter()
                .find(|a| a.name == "execution_period_ms")
                .and_then(|a| a.default.as_ref())
                .and_then(|d| d.trim().parse::<u64>().ok())
                .map(|ms| ms * 1000);

            // Compute max WCET sum across all states (worst case = state with highest total)
            let mut max_state_wcet: u64 = 0;
            for state in &sm.states {
                let mut state_wcet: u64 = 0;
                for action in state
                    .entry_actions
                    .iter()
                    .chain(state.do_actions.iter())
                    .chain(state.exit_actions.iter())
                {
                    if let Some(wcet) = action.wcet_us {
                        state_wcet += wcet;
                    }
                }
                if state_wcet > max_state_wcet {
                    max_state_wcet = state_wcet;
                }
            }

            actors.push(ActorTiming {
                name: part.name.clone(),
                core,
                priority,
                max_wcet_us: max_state_wcet,
                period_us,
            });
        }
    }

    if actors.is_empty() {
        return;
    }

    // Group by core
    let mut cores: HashMap<u64, Vec<&ActorTiming>> = HashMap::new();
    for actor in &actors {
        cores.entry(actor.core).or_default().push(actor);
    }

    // Sort core IDs for deterministic output
    let mut core_ids: Vec<u64> = cores.keys().copied().collect();
    core_ids.sort_unstable();

    println!("\n  Non-preemptive response-time analysis:");
    for core_id in &core_ids {
        let core_actors = &cores[core_id];
        let peers: Vec<(u64, u64)> = core_actors
            .iter()
            .map(|a| (a.max_wcet_us, a.priority))
            .collect();

        println!("  Core {core_id}:");
        for actor in core_actors {
            let (response, blocking) =
                compute_response_time(actor.max_wcet_us, actor.priority, &peers);

            if let Some(period) = actor.period_us {
                if response > period {
                    println!(
                        "    WARNING: {} (prio={}): C={} + B={} = R={}us > T={}us",
                        actor.name, actor.priority, actor.max_wcet_us, blocking, response, period,
                    );
                } else {
                    println!(
                        "    {} (prio={}): C={} + B={} = R={}us — OK (T={}us, margin={}us)",
                        actor.name,
                        actor.priority,
                        actor.max_wcet_us,
                        blocking,
                        response,
                        period,
                        period - response,
                    );
                }
            } else {
                println!(
                    "    {} (prio={}): C={} + B={} = R={}us — event-driven",
                    actor.name, actor.priority, actor.max_wcet_us, blocking, response,
                );
            }
        }
    }
}

struct Cli {
    input_files: Vec<PathBuf>,
    output_dir: PathBuf,
    mcrl2_dir: Option<PathBuf>,
}

fn parse_cli(args: &[String]) -> Cli {
    if args.len() < 2 {
        eprintln!(
            "Usage: sysml-codegen <file.sysml>... --output-dir <dir> [--mcrl2-dir <dir>]\n\
             Or: task generate (passes paths from Taskfile)"
        );
        std::process::exit(1);
    }

    let mut input_files = Vec::new();
    let mut output_dir: Option<PathBuf> = None;
    let mut mcrl2_dir: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--output-dir" {
            i += 1;
            if i < args.len() {
                output_dir = Some(PathBuf::from(&args[i]));
            }
        } else if args[i] == "--mcrl2-dir" {
            i += 1;
            if i < args.len() {
                mcrl2_dir = Some(PathBuf::from(&args[i]));
            }
        } else {
            input_files.push(PathBuf::from(&args[i]));
        }
        i += 1;
    }

    let output_dir = output_dir.unwrap_or_else(|| {
        eprintln!("error: --output-dir is required");
        std::process::exit(1);
    });

    Cli {
        input_files,
        output_dir,
        mcrl2_dir,
    }
}

fn write_package_mcrl2(package: &ast::Package, mcrl2_output: &Path) {
    std::fs::create_dir_all(mcrl2_output).expect("create mCRL2 output dir");

    let mcrl2_specs = codegen::generate_mcrl2(package);
    for (part_name, mcrl2_content, props) in &mcrl2_specs {
        let mcrl2_file = mcrl2_output.join(format!("{part_name}.mcrl2"));
        std::fs::write(&mcrl2_file, mcrl2_content).expect("write mCRL2 file");
        println!("Generated mCRL2: {}", mcrl2_file.display());

        for (prop_name, mcf_content) in props {
            let mcf_file = mcrl2_output.join(format!("{prop_name}.mcf"));
            std::fs::write(&mcf_file, mcf_content).expect("write mcf file");
            println!("Generated mcf:   {}", mcf_file.display());
        }
    }

    let timed_specs = codegen::generate_timed_mcrl2(package);
    for (timed_name, mcrl2_content, props) in &timed_specs {
        let mcrl2_file = mcrl2_output.join(format!("{timed_name}.mcrl2"));
        std::fs::write(&mcrl2_file, mcrl2_content).expect("write timed mCRL2 file");
        println!("Generated timed mCRL2: {}", mcrl2_file.display());

        for (prop_name, mcf_content) in props {
            let mcf_file = mcrl2_output.join(format!("{prop_name}.mcf"));
            std::fs::write(&mcf_file, mcf_content).expect("write timed mcf file");
            println!("Generated timed mcf:   {}", mcf_file.display());
        }
    }
}

fn write_actor_packages(cli: &Cli, packages: &[ast::Package], rust_modules: &mut Vec<String>) {
    for package in packages {
        println!("\nGenerating: {}", package.name);

        if package.is_composition_only() {
            println!("  Skipping actor mod (composition-only package)");
        } else {
            let rust_code = codegen::generate(package);
            let mod_name = package.name.to_lowercase();
            let output_file = cli.output_dir.join(format!("{mod_name}.rs"));
            std::fs::write(&output_file, &rust_code).expect("write output file");
            rust_modules.push(mod_name);
            println!("Generated: {}", output_file.display());
        }

        if let Some(ref mcrl2_output) = cli.mcrl2_dir {
            write_package_mcrl2(package, mcrl2_output);
        }
    }
}

fn write_task_loops(cli: &Cli, packages: &[ast::Package], rust_modules: &mut Vec<String>) {
    let tasks_code = codegen::generate_task_loops(packages);
    let tasks_file = cli.output_dir.join("tasks.rs");
    std::fs::write(&tasks_file, &tasks_code).expect("write tasks.rs");
    rust_modules.push("tasks".into());
    println!("Generated tasks: {}", tasks_file.display());
}

fn write_system_composition(cli: &Cli, packages: &[ast::Package], rust_modules: &mut Vec<String>) {
    let parts_by_name: HashMap<String, &ast::PartDef> = packages
        .iter()
        .flat_map(|p| p.parts.iter())
        .map(|pd| (pd.name.clone(), pd))
        .collect();
    let ports_by_package: Vec<&ast::Port> =
        packages.iter().flat_map(|p| p.port_defs.iter()).collect();

    for package in packages {
        for part in &package.parts {
            if part.part_instances.is_empty() || part.connections.is_empty() {
                continue;
            }
            println!("\nSystem detected: {}", part.name);

            let mut resolved: HashMap<String, &ast::PartDef> = HashMap::new();
            for inst in &part.part_instances {
                if let Some(pd) = parts_by_name.get(&inst.typ) {
                    resolved.insert(inst.name.clone(), pd);
                } else {
                    eprintln!(
                        "  Warning: part type '{}' not found for instance '{}'",
                        inst.typ, inst.name
                    );
                }
            }

            if let Some(ref mcrl2_output) = cli.mcrl2_dir {
                std::fs::create_dir_all(mcrl2_output).expect("create mCRL2 output dir");
                let (mcrl2_content, props) =
                    mcrl2_compose::render_composed_mcrl2(part, &resolved, &ports_by_package);
                let mcrl2_file = mcrl2_output.join(format!("{}.mcrl2", part.name));
                std::fs::write(&mcrl2_file, &mcrl2_content).expect("write composed mCRL2 file");
                println!("Generated composed mCRL2: {}", mcrl2_file.display());
                for (prop_name, mcf_content) in &props {
                    let mcf_file = mcrl2_output.join(format!("{prop_name}.mcf"));
                    std::fs::write(&mcf_file, mcf_content).expect("write composed mcf file");
                    println!("Generated composed mcf:  {}", mcf_file.display());
                }
            }

            let channels_code = codegen::generate_system_channels(part, &resolved);
            let channels_file = cli.output_dir.join("channels.rs");
            std::fs::write(&channels_file, &channels_code).expect("write channels.rs");
            rust_modules.push("channels".into());
            println!("Generated channels: {}", channels_file.display());
        }
    }
}

fn generate_packages(cli: &Cli) {
    std::fs::create_dir_all(&cli.output_dir).expect("create output dir");
    codegen::clear_generated_rust(&cli.output_dir);
    let mut rust_modules: Vec<String> = Vec::new();

    let mut packages = Vec::new();
    for input in &cli.input_files {
        println!("\nParsing: {}", input.display());
        let content = std::fs::read_to_string(input).expect("read input file");
        let filename = input.to_str().unwrap_or("unknown");
        packages.push(parser::parse_sysml(&content, filename));
    }

    write_actor_packages(cli, &packages, &mut rust_modules);
    check_schedulability(&packages.iter().collect::<Vec<_>>());
    write_task_loops(cli, &packages, &mut rust_modules);
    write_system_composition(cli, &packages, &mut rust_modules);

    codegen::write_mod_rs(&cli.output_dir, &rust_modules);
    println!(
        "Generated mod.rs: {}",
        cli.output_dir.join("mod.rs").display()
    );

    println!("\nCode generation complete!");
    println!("  Output directory: {}", cli.output_dir.display());
    if let Some(ref mcrl2_output) = cli.mcrl2_dir {
        println!("  mCRL2 directory:  {}", mcrl2_output.display());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cli = parse_cli(&args);
    generate_packages(&cli);
}
