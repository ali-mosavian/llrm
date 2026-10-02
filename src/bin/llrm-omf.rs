//! BC objects recompiled through the rich route: `llrm-omf OBJ... -o OUT`.

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    ExitCode::from(u8::try_from(llrm_bcdriver::main(&arguments)).unwrap_or(1))
}
