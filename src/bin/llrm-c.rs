//! `python -m qbopt.cfront`, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = llrm_core::support::debug::run_main(|| llrm_c::compile::main(&argv));
    std::process::exit(code);
}
