//! `llrm-mir FILE`: reads MIR and writes it back, as `tools/mir-oracle.sh`
//! compares with LLVM's own reading.

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: llrm-mir FILE");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::from(2);
        }
    };
    match llrm_mir::parse::module(&text) {
        Ok(module) => {
            print!("{}", llrm_mir::print::module(&module));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{path}:{error}");
            ExitCode::FAILURE
        }
    }
}
