#![allow(dead_code)]

mod ast;
mod codegen;
mod expr;
mod parser;
mod render;
mod rust_ast;
mod tla_expr;
mod tla_render;
mod translate;

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

    for input in &input_files {
        println!("\nProcessing: {}", input.display());

        let content = std::fs::read_to_string(input).expect("read input file");
        let filename = input.to_str().unwrap_or("unknown");

        let package = parser::parse_sysml(&content, filename);
        let rust_code = codegen::generate(&package);

        let output_file = output_dir.join(format!("{}.rs", package.name.to_lowercase()));
        std::fs::write(&output_file, &rust_code).expect("write output file");

        println!("Generated: {}", output_file.display());

        // Generate TLA+ specs if --tla-dir is set
        if let Some(ref tla_output) = tla_dir {
            std::fs::create_dir_all(tla_output).expect("create TLA+ output dir");

            let tla_specs = codegen::generate_tla(&package);
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

    println!("\nCode generation complete!");
    println!("  Output directory: {}", output_dir.display());
    if let Some(ref tla_output) = tla_dir {
        println!("  TLA+ directory:   {}", tla_output.display());
    }
}
