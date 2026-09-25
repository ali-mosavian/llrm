//! `python -m qbopt.frontend.qb`, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = llrm_qb::cli::main(&argv);
    llrm_core::support::debug::report_times();
    std::process::exit(code);
}
