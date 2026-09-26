//! hir-mir HIR.json: the MIR each HIR module emits, on stdout; each
//! refusal, and anything the verifier rejects, on stderr.

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: hir-mir HIR.json");
        return ExitCode::from(2);
    };
    let program = match std::fs::read_to_string(&path).map_err(|error| error.to_string()).and_then(|text| llrm_hir::codec::decode(&text).map_err(|error| error.to_string())) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::from(2);
        }
    };
    let mut invalid = false;
    for emitted in llrm_hir::mir::emit(&program) {
        print!("{}", llrm_mir::print::module(&emitted.module));
        for (name, why) in &emitted.refused {
            eprintln!("refused @{name}: {why}");
        }
        for problem in llrm_mir::verify::verify(&emitted.module) {
            eprintln!("invalid: {problem}");
            invalid = true;
        }
    }
    if invalid { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
