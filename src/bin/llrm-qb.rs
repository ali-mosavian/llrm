//! `python -m qbopt.frontend.qb`, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(llrm::frontend::qbc::main::main(&argv));
}
