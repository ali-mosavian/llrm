//! Runs a modern-language module's entry on the host HIR interpreter.
//!
//!   llrm-run SOURCE.mod [ENTRY] [INTEGER...]

use std::process::ExitCode;

use llrm::hir::codec;
use llrm::hir::execute;
use llrm::hir::model::Number;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some(input) = arguments.first() else {
        eprintln!("usage: llrm-run SOURCE.mod [ENTRY] [INTEGER...]");
        return ExitCode::from(2);
    };
    let entry = arguments.get(1).map_or("main", String::as_str);
    let Ok(values) = arguments
        .iter()
        .skip(2)
        .map(|one| one.parse().map(Number::Int))
        .collect::<Result<Vec<_>, _>>()
    else {
        eprintln!("llrm-run: arguments are integers");
        return ExitCode::from(2);
    };
    let hir = match llrm::frontends::modern::compile_file(std::path::Path::new(input)) {
        Ok(hir) => hir,
        Err((path, error)) => {
            eprintln!(
                "{}:{}:{}: {}",
                path.display(),
                error.span.line,
                error.span.column,
                error.message
            );
            return ExitCode::FAILURE;
        }
    };
    let result = codec::decode(&hir)
        .map_err(|error| error.to_string())
        .and_then(|program| {
            execute::run(&program, entry, &values).map_err(|error| error.to_string())
        });
    match result {
        Ok(executed) => {
            print!("{}", executed.output);
            if let Some(panic) = executed.panic {
                eprintln!("panic: {panic}");
                return ExitCode::FAILURE;
            }
            if let Some(value) = executed.value {
                eprintln!("{entry} returned {value:?}");
            }
            if executed.leaked != 0 {
                eprintln!("llrm-run: {} heap buffers leaked", executed.leaked);
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("llrm-run: {error}");
            ExitCode::FAILURE
        }
    }
}
