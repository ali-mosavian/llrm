//! `python -m qbopt.cfront`, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = llrm::frontends::c::compile::main(&argv);
    llrm::support::debug::report_times();
    std::process::exit(code);
}
