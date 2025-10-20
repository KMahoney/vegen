use ariadne::{Report, ReportKind, Source};
use clap::{command, Arg, ArgAction, Command};
use itertools::Itertools;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::exit,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

mod ast;
mod ast_query;
mod attribute_types;
mod builtins;
mod compile;
mod emit;
mod error;
mod expr;
mod ir;
mod lsp;
mod parser;
mod ts_type;
mod ts_util;
mod type_system;

fn compile_all(vg_files: &[PathBuf], quiet: bool) -> Result<String, ()> {
    if !quiet {
        eprintln!("Found {} .vg files.", vg_files.len());
    }

    // Collect filenames for SourceId indexing
    let filenames: Vec<String> = vg_files
        .iter()
        .map(|p| p.to_str().unwrap_or("<non-utf8>").to_string())
        .collect();

    // Parse each .vg file and collect nodes
    let mut nodes = Vec::new();
    let mut file_contents = Vec::new();

    for (i, file) in vg_files.iter().enumerate() {
        let file_str = &filenames[i];

        let file_content = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to read '{}': {}", file_str, e);
                return Err(());
            }
        };
        file_contents.push(file_content.clone());

        match parser::parse_template(&file_content, i) {
            Ok(file_nodes) => {
                nodes.extend(file_nodes);
            }
            Err(errors) => {
                for e in errors {
                    let mut report =
                        Report::build(ReportKind::Error, (file_str, e.main_span.into_range()))
                            .with_message(&e.message);
                    for (span, label_msg) in &e.labels {
                        report = report.with_label(
                            ariadne::Label::new((file_str, span.into_range()))
                                .with_message(label_msg)
                                .with_color(ariadne::Color::Red),
                        );
                    }
                    report
                        .finish()
                        .eprint((file_str, Source::from(file_content.clone())))
                        .unwrap();
                }
                return Err(());
            }
        }
    }

    // Compile nodes into TypeScript
    match compile::compile(&nodes) {
        Ok(output) => Ok(output.code),
        Err(e) => {
            // Use SourceId from error span to lookup filename
            let source_id = e.main_span.context;
            let file_str = filenames[source_id].as_str();
            let file_content = file_contents[source_id].as_str();

            let mut report = Report::build(ReportKind::Error, (file_str, e.main_span.into_range()))
                .with_message(&e.message);
            for (span, label_msg) in &e.labels {
                report = report.with_label(
                    ariadne::Label::new((file_str, (*span).into_range()))
                        .with_message(label_msg)
                        .with_color(ariadne::Color::Red),
                );
            }
            report
                .finish()
                .eprint((file_str, Source::from(file_content)))
                .unwrap();
            Err(())
        }
    }
}

fn write_output(output_file: Option<&str>, output: &str) {
    if let Some(out) = output_file {
        std::fs::write(out, output).expect("Failed to write output file");
        eprintln!("Generated TypeScript written to '{}'", out);
    } else {
        println!("{}", output);
        eprintln!("Generated TypeScript written to stdout.");
    }
}

fn main() {
    let matches = command!()
        .subcommand_negates_reqs(true)
        .arg(
            Arg::new("input")
                .help("One or more .vg template files")
                .required(true)
                .num_args(1..)
                .value_name("INPUT"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .help("File for generated TypeScript (default: stdout)")
                .value_name("OUTPUT")
                .required(false),
        )
        .arg(
            Arg::new("watch")
                .short('w')
                .long("watch")
                .help("Watch input .vg files for changes and recompile automatically")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .action(ArgAction::SetTrue),
        )
        .subcommand(
            Command::new("lsp").about("Run the VeGen language server (LSP) over stdin/stdout"),
        )
        .get_matches();

    if matches.subcommand_matches("lsp").is_some() {
        let exit_code = lsp::run();
        exit(exit_code);
    }

    let input_files = matches
        .get_many::<String>("input")
        .expect("input file(s) required")
        .map(|s| s.to_string())
        .collect_vec();

    let output_file = matches.get_one::<String>("output");
    let watch = matches.get_flag("watch");
    let quiet = matches.get_flag("quiet");

    if !quiet {
        eprintln!("Input file(s): {:?}", input_files);
        if let Some(out) = output_file {
            eprintln!("Output file: {}", out);
        } else {
            eprintln!("No output file specified, using stdout.");
        }
    }

    // Collect .vg files from input arguments
    let mut vg_files = Vec::new();
    for file in &input_files {
        let path = Path::new(file);
        if path.is_file() {
            vg_files.push(path.to_path_buf());
        } else {
            eprintln!("Input '{}' is not a valid file", file);
        }
    }

    if vg_files.is_empty() {
        eprintln!("No valid .vg files provided.");
        std::process::exit(1);
    }

    if !watch {
        match compile_all(&vg_files, quiet) {
            Ok(output) => {
                write_output(output_file.map(|s| s.as_str()), &output);
            }
            Err(()) => exit(1),
        }
        return;
    }

    // Watch mode: polling-based simple watcher (no extra dependencies)
    eprintln!(
        "Watch mode enabled. Watching {} file(s) for changes. Press Ctrl-C to exit.",
        vg_files.len()
    );

    // Initial build (do not exit on error, continue watching)
    match compile_all(&vg_files, quiet) {
        Ok(output) => write_output(output_file.map(|s| s.as_str()), &output),
        Err(()) => eprintln!("Initial build failed. Watching for further changes..."),
    }

    // Track last modification times
    let mut last_mod: HashMap<PathBuf, SystemTime> = HashMap::new();
    for f in &vg_files {
        let t = std::fs::metadata(f)
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH);
        last_mod.insert(f.clone(), t);
    }

    // Polling loop with a small debounce
    loop {
        let mut changed = false;

        for f in &vg_files {
            let current = std::fs::metadata(f)
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);

            let prev = last_mod.get(f).cloned().unwrap_or(UNIX_EPOCH);
            if current > prev {
                last_mod.insert(f.clone(), current);
                changed = true;
            }
        }

        if changed {
            eprintln!("Change detected. Recompiling...");
            match compile_all(&vg_files, quiet) {
                Ok(output) => {
                    write_output(output_file.map(|s| s.as_str()), &output);
                    eprintln!("Rebuild complete.");
                }
                Err(()) => {
                    eprintln!("Build failed. Watching for further changes...");
                }
            }
            // Debounce a little after a rebuild to avoid rapid re-triggers
            thread::sleep(Duration::from_millis(150));
        }

        thread::sleep(Duration::from_millis(250));
    }
}
