//! `tools/modernexe.py`'s compile to an object, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = llrm_core::support::debug::run_main(|| llrm_nib::cli::main(&argv));
    std::process::exit(code);
}
