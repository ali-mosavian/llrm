//! Compile Nib to an object.

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(why) = llrm_mir::dumpdir::refuse(&argv) {
        eprintln!("llrm-nib: {why}");
        std::process::exit(2);
    }
    let code = llrm_core::support::debug::run_main(|| llrm_nib::cli::main(&argv));
    llrm_core::support::debug::report_checks(&[("STALE", llrm_mir::passes::stale_checks_run())]);
    std::process::exit(code);
}
