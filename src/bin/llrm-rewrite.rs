//! `python -m qbopt.rewrite`. Temporary name until it merges into `llrm-omf`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match llrm::rewrite::main(&argv) {
        Ok(status) => ExitCode::from(u8::try_from(status).unwrap_or(1)),
        Err(raised) => {
            eprintln!("Traceback (most recent call last):\n{}: {}", raised.kind, raised.message);
            ExitCode::from(1)
        }
    }
}
