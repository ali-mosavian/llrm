//! `python -m qbopt.frontend.qb`, ported.

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = llrm_core::support::debug::run_main(|| llrm_qb::cli::main(&argv));
    std::process::exit(code);
}
