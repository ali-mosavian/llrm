use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use qbfront::{parse, Dialect};

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let mut dialect = Dialect::VbDos;
    let mut runtime = "vbdos".to_string();
    let mut row_major = false;
    let mut huge_arrays = false;
    let mut checked_arrays = false;
    let mut unchecked_bounds = false;
    let mut mbf = false;
    let mut alternate_math = false;
    let mut whole_program = false;
    let mut syntax = false;
    let mut include_dirs = Vec::new();
    let mut dump_source = None;
    let mut input = None;
    while let Some(argument) = arguments.next() {
        if argument == "--dialect" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --dialect requires a value");
                return ExitCode::from(2);
            };
            let Some(found) = Dialect::parse(&value) else {
                eprintln!("qbfront: unknown dialect {value:?}");
                return ExitCode::from(2);
            };
            dialect = found;
        } else if argument == "--runtime" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --runtime requires a value");
                return ExitCode::from(2);
            };
            if !matches!(value.as_str(), "qb45" | "pds71" | "vbdos") {
                eprintln!("qbfront: unknown runtime {value:?}");
                return ExitCode::from(2);
            }
            runtime = value;
        } else if argument == "--syntax" {
            syntax = true;
        } else if argument == "--huge-arrays" {
            huge_arrays = true;
        } else if argument == "--checked-arrays" {
            checked_arrays = true;
        } else if argument == "--unchecked-bounds" {
            unchecked_bounds = true;
        } else if argument == "--mbf" {
            mbf = true;
        } else if argument == "--alternate-math" {
            alternate_math = true;
        } else if argument == "--whole-program" {
            whole_program = true;
        } else if argument == "--array-order" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --array-order requires column-major or row-major");
                return ExitCode::from(2);
            };
            match value.as_str() {
                "column-major" => row_major = false,
                "row-major" => row_major = true,
                _ => {
                    eprintln!("qbfront: unknown array order {value:?}");
                    return ExitCode::from(2);
                }
            }
        } else if argument == "--include" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --include requires a directory");
                return ExitCode::from(2);
            };
            include_dirs.push(PathBuf::from(value));
        } else if argument == "--dump-source" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --dump-source requires a file");
                return ExitCode::from(2);
            };
            dump_source = Some(PathBuf::from(value));
        } else if input.replace(argument).is_some() {
            eprintln!("qbfront: expected one input file");
            return ExitCode::from(2);
        }
    }
    let Some(input) = input else {
        eprintln!(
            "usage: qbfront [--dialect PROFILE] [--runtime PROFILE] [--array-order column-major|row-major] [--huge-arrays] [--checked-arrays] [--unchecked-bounds] [--whole-program] [--include DIR] [--syntax] FILE"
        );
        return ExitCode::from(2);
    };
    let source = match qbfront::source::load_with_map(std::path::Path::new(&input), &include_dirs) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("qbfront: {input}: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(path) = dump_source {
        if let Err(error) = fs::write(&path, &source.text) {
            eprintln!("qbfront: {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    }
    match parse(&source.text, dialect) {
        Ok(module) => {
            if syntax {
                println!(
                    "ok: {} module statements, {} procedures",
                    module.statements.len(),
                    module.procedures.len()
                );
                ExitCode::SUCCESS
            } else {
                let name = std::path::Path::new(&input)
                    .file_stem()
                    .and_then(|one| one.to_str())
                    .unwrap_or("module");
                let options = qbfront::semantic::Options {
                    row_major,
                    huge_arrays,
                    checked_arrays,
                    unchecked_bounds,
                    mbf,
                    alternate_math,
                    whole_program,
                };
                match qbfront::semantic::compile_with_options(&module, name, dialect, &runtime, &options) {
                    Ok(hir) => {
                        print!("{hir}");
                        ExitCode::SUCCESS
                    }
                    Err(error) => {
                        eprintln!("{input}: {}", error.message);
                        ExitCode::FAILURE
                    }
                }
            }
        }
        Err(error) => {
            let location = source.location(error.span.line);
            let path = location
                .map(|location| location.path.display().to_string())
                .unwrap_or_else(|| input.clone());
            let line = location.map_or(error.span.line, |location| location.line);
            eprintln!(
                "{path}:{}:{}: {}",
                line,
                error.span.start + 1,
                error.message
            );
            ExitCode::FAILURE
        }
    }
}
