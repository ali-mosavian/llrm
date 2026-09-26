//! Dump the generated parser's source, typed action stream, and syntax tree.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use qbfront::generated_parser::parse_vertical_slice;
use qbfront::{source, Dialect};

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let Some(dialect_name) = arguments.next() else {
        return usage();
    };
    let Some(dialect) = Dialect::parse(&dialect_name) else {
        eprintln!("qbparse-stages: unknown dialect {dialect_name:?}");
        return ExitCode::from(2);
    };
    let Some(input_name) = arguments.next() else {
        return usage();
    };
    let Some(output_name) = arguments.next() else {
        return usage();
    };
    if arguments.next().is_some() {
        return usage();
    }

    let input = PathBuf::from(input_name);
    let output = PathBuf::from(output_name);
    let source =
        match source::load_with_map(&input, &[input.parent().unwrap_or(Path::new(".")).into()]) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("qbparse-stages: {}: {error}", input.display());
                return ExitCode::FAILURE;
            }
        };
    let parsed = match parse_vertical_slice(&source.text, dialect) {
        Ok(parsed) => parsed,
        Err(error) => {
            let location = source.location(error.span.line);
            let path = location.map_or_else(|| input.as_path(), |location| location.path.as_path());
            let line = location.map_or(error.span.line, |location| location.line);
            eprintln!(
                "qbparse-stages: {}:{}:{}: {}",
                path.display(),
                line,
                error.span.start + 1,
                error.message
            );
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = fs::create_dir_all(&output)
        .and_then(|()| fs::write(output.join("00-input.bas"), &source.text))
        .and_then(|()| {
            fs::write(
                output.join("10-actions.txt"),
                format!("{:#?}\n", parsed.actions),
            )
        })
        .and_then(|()| fs::write(output.join("20-ast.txt"), format!("{:#?}\n", parsed.module)))
    {
        eprintln!("qbparse-stages: {}: {error}", output.display());
        return ExitCode::FAILURE;
    }

    println!(
        "{} -> {} ({} actions, {} module statements, {} procedures)",
        input.display(),
        output.display(),
        parsed.actions.len(),
        parsed.module.statements.len(),
        parsed.module.procedures.len()
    );
    ExitCode::SUCCESS
}

fn usage() -> ExitCode {
    eprintln!("usage: qbparse-stages DIALECT INPUT OUTPUT-DIRECTORY");
    ExitCode::from(2)
}
