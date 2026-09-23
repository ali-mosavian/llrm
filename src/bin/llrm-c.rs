//! `python -m qbopt.cfront`, ported.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(llrm::frontends::c::compile::main(&argv));
}
