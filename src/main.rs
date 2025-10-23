use crate::error::Error;
use crate::template::{load_ordered_views, SourceMap, TemplatePath, TemplateResolver};
use ariadne::{Color, Report, ReportKind, Source};
use clap::{command, Arg, ArgAction, Command};
use itertools::Itertools;
use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    process::exit,
    sync::Arc,
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
mod graph;
mod expr;
mod ir;
mod lsp;
mod parser;
mod template;
mod ts_type;
mod ts_util;
mod type_system;

struct DiskResolver;

impl TemplateResolver for DiskResolver {
    fn resolve(&mut self, path: &TemplatePath) -> io::Result<Arc<str>> {
        let text = std::fs::read_to_string(path.as_ref())?;
        Ok(Arc::from(text))
    }
}

fn compile_all(vg_files: &[PathBuf], quiet: bool) -> Result<(String, Vec<PathBuf>), ()> {
    if !quiet {
        eprintln!("Found {} .vg files.", vg_files.len());
    }

    let mut resolver = DiskResolver;
    let mut sources = SourceMap::new();
    let mut ordered_views = Vec::new();
    let mut seen_views = HashSet::new();

    for file in vg_files {
        let template_path: TemplatePath = Arc::new(file.clone());
        match load_ordered_views(template_path, &mut resolver, &mut sources) {
            Ok(views) => {
                for view in views {
                    if seen_views.insert(view.name.clone()) {
                        ordered_views.push(view);
                    }
                }
            }
            Err(errors) => {
                for error in errors {
                    report_error(&sources, &error);
                }
                return Err(());
            }
        }
    }

    match compile::compile_views(&ordered_views) {
        Ok(output) => {
            let watched_paths = sources
                .iter()
                .map(|(_, record)| record.path.as_ref().clone())
                .collect();
            Ok((output.code, watched_paths))
        }
        Err(error) => {
            report_error(&sources, &error);
            Err(())
        }
    }
}

fn report_error(sources: &SourceMap, error: &Error) {
    if let Some(record) = sources.record(error.main_span.context) {
        let filename = record.path.as_ref().to_string_lossy().to_string();
        let mut report = Report::build(
            ReportKind::Error,
            (filename.clone(), error.main_span.into_range()),
        )
        .with_message(&error.message);

        for (span, label_msg) in &error.labels {
            if let Some(label_record) = sources.record(span.context) {
                let label_filename = label_record.path.as_ref().to_string_lossy().to_string();
                if label_filename == filename {
                    report = report.with_label(
                        ariadne::Label::new((label_filename, (*span).into_range()))
                            .with_message(label_msg)
                            .with_color(Color::Red),
                    );
                }
            }
        }

        if let Err(report_err) = report
            .finish()
            .eprint((filename, Source::from(record.text.as_ref().to_string())))
        {
            eprintln!("Failed to emit diagnostic: {}", report_err);
        }
    } else {
        eprintln!("{}", error.message);
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
            Ok((output, _deps)) => {
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
    let mut watched_paths = vg_files.clone();
    match compile_all(&vg_files, quiet) {
        Ok((output, deps)) => {
            write_output(output_file.map(|s| s.as_str()), &output);
            watched_paths = deps;
        }
        Err(()) => eprintln!("Initial build failed. Watching for further changes..."),
    }

    // Track last modification times
    let mut last_mod: HashMap<PathBuf, SystemTime> = HashMap::new();
    for f in &watched_paths {
        let t = std::fs::metadata(f)
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH);
        last_mod.insert(f.clone(), t);
    }

    // Polling loop with a small debounce
    loop {
        let mut changed = false;

        for f in &watched_paths {
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
                Ok((output, deps)) => {
                    write_output(output_file.map(|s| s.as_str()), &output);
                    eprintln!("Rebuild complete.");

                    let mut deps_set: HashSet<PathBuf> = deps.into_iter().collect();
                    for original in &vg_files {
                        deps_set.insert(original.clone());
                    }
                    watched_paths = deps_set.into_iter().collect();

                    last_mod.retain(|path, _| watched_paths.contains(path));
                    for path in &watched_paths {
                        let t = std::fs::metadata(path)
                            .and_then(|m| m.modified())
                            .unwrap_or(UNIX_EPOCH);
                        last_mod.insert(path.clone(), t);
                    }
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
