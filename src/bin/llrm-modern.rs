//! `tools/modernexe.py`'s compile to an object, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(llrm::frontends::modern::main::main(&argv));
}
