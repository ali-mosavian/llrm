//! `tools/modernexe.py`'s compile to an object, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(llrm::frontend::modern::main::main(&argv));
}
