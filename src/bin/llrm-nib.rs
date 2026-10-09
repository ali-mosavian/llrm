//! `tools/modernexe.py`'s compile to an object, ported.

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = llrm_core::support::debug::run_main(|| llrm_nib::cli::main(&argv));
    std::process::exit(code);
}
