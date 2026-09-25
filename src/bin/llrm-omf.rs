//! `python -m qbopt.rewrite`, and with `--dump DIR`, `tools/stages.py --dump`.

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.iter().any(|one| one == "--dump") {
        return match llrm_core::tools::stages::main(&arguments) {
            Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
            Err(message) => {
                eprintln!("llrm-omf: {message}");
                ExitCode::FAILURE
            }
        };
    }
    match llrm_core::rewrite::main(&arguments) {
        Ok(status) => ExitCode::from(u8::try_from(status).unwrap_or(1)),
        Err(raised) => {
            eprintln!("Traceback (most recent call last):\n{}", raised.traceback());
            ExitCode::from(1)
        }
    }
}
