//! `tools/modernexe.py`'s compile to an object, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = llrm::frontends::modern::main::main(&argv);
    llrm::support::debug::report_times();
    std::process::exit(code);
}
