//! Compile QuickBASIC to an object.

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if llrm_codegen::support::version::asked(
        &argv,
        "llrm-qb",
        env!("LLRM_BUILD_SHA"),
        env!("LLRM_BUILD_DIRTY"),
        env!("LLRM_BUILD_INCREMENTAL"),
    ) {
        return;
    }
    if let Err(why) = llrm_mir::dumpdir::refuse(&argv) {
        eprintln!("llrm-qb: {why}");
        std::process::exit(2);
    }
    let code = llrm_codegen::support::debug::run_main(|| llrm_qb::cli::main(&argv));
    llrm_codegen::support::debug::report_checks(&[("STALE", llrm_mir::passes::stale_checks_run())]);
    std::process::exit(code);
}
