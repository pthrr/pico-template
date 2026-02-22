#![allow(dead_code)]

mod ast;
mod codegen;
mod expr;
mod parser;
mod render;
mod rust_ast;
mod tla_compose;
mod tla_expr;
mod tla_render;
mod translate;

use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: sysml-codegen <file.sysml>... [--output-dir <dir>] [--tla-dir <dir>]");
        std::process::exit(1);
    }

    let mut input_files = Vec::new();
    let mut output_dir = PathBuf::from("generated");
    let mut tla_dir: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--output-dir" {
            i += 1;
            if i < args.len() {
                output_dir = PathBuf::from(&args[i]);
            }
        } else if args[i] == "--tla-dir" {
            i += 1;
            if i < args.len() {
                tla_dir = Some(PathBuf::from(&args[i]));
            }
        } else {
            input_files.push(PathBuf::from(&args[i]));
        }
        i += 1;
    }

    std::fs::create_dir_all(&output_dir).expect("create output dir");

    // Phase 1: Parse all input files
    let mut packages = Vec::new();
    for input in &input_files {
        println!("\nParsing: {}", input.display());
        let content = std::fs::read_to_string(input).expect("read input file");
        let filename = input.to_str().unwrap_or("unknown");
        packages.push(parser::parse_sysml(&content, filename));
    }

    // Phase 2: Per-file generation (unchanged behavior)
    for package in &packages {
        println!("\nGenerating: {}", package.name);

        let rust_code = codegen::generate(package);
        let output_file = output_dir.join(format!("{}.rs", package.name.to_lowercase()));
        std::fs::write(&output_file, &rust_code).expect("write output file");
        println!("Generated: {}", output_file.display());

        if let Some(ref tla_output) = tla_dir {
            std::fs::create_dir_all(tla_output).expect("create TLA+ output dir");

            let tla_specs = codegen::generate_tla(package);
            for (part_name, tla_content, cfg_content) in &tla_specs {
                let tla_file = tla_output.join(format!("{part_name}.tla"));
                let cfg_file = tla_output.join(format!("{part_name}.cfg"));

                std::fs::write(&tla_file, tla_content).expect("write TLA+ file");
                std::fs::write(&cfg_file, cfg_content).expect("write TLA+ cfg file");

                println!("Generated TLA+: {}", tla_file.display());
                println!("Generated cfg:  {}", cfg_file.display());
            }
        }
    }

    // Phase 3: System composition — detect system part defs and generate composed specs
    let all_part_defs: HashMap<String, &ast::PartDef> = packages.iter()
        .flat_map(|p| p.parts.iter())
        .map(|pd| (pd.name.clone(), pd))
        .collect();

    let all_port_defs: Vec<&ast::Port> = packages.iter()
        .flat_map(|p| p.port_defs.iter())
        .collect();

    for package in &packages {
        for part in &package.parts {
            if part.part_instances.is_empty() || part.connections.is_empty() {
                continue;
            }

            println!("\nSystem detected: {}", part.name);

            // Resolve part instances to their PartDefs
            let mut resolved: HashMap<String, &ast::PartDef> = HashMap::new();
            for inst in &part.part_instances {
                if let Some(pd) = all_part_defs.get(&inst.typ) {
                    resolved.insert(inst.name.clone(), pd);
                } else {
                    eprintln!("  Warning: part type '{}' not found for instance '{}'",
                              inst.typ, inst.name);
                }
            }

            // Generate composed TLA+ spec
            if let Some(ref tla_output) = tla_dir {
                std::fs::create_dir_all(tla_output).expect("create TLA+ output dir");

                let (tla_content, cfg_content) =
                    tla_compose::render_composed_tla(part, &resolved, &all_port_defs);

                let tla_file = tla_output.join(format!("{}.tla", part.name));
                let cfg_file = tla_output.join(format!("{}.cfg", part.name));

                std::fs::write(&tla_file, &tla_content).expect("write composed TLA+ file");
                std::fs::write(&cfg_file, &cfg_content).expect("write composed TLA+ cfg file");

                println!("Generated composed TLA+: {}", tla_file.display());
                println!("Generated composed cfg:  {}", cfg_file.display());
            }

            // Generate channel declarations (message types are in per-actor files)
            let channels_code = codegen::generate_system_channels(part, &resolved);
            if !channels_code.is_empty() {
                println!("\nGenerated define_channels! for {}:\n{channels_code}", part.name);
            }
        }
    }

    println!("\nCode generation complete!");
    println!("  Output directory: {}", output_dir.display());
    if let Some(ref tla_output) = tla_dir {
        println!("  TLA+ directory:   {}", tla_output.display());
    }
}
