//! Compile LLVM IR to an object.

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if llrm_codegen::support::version::asked(&argv, "llrm-llc", env!("LLRM_BUILD_SHA"), env!("LLRM_BUILD_DIRTY")) {
        return;
    }
    let status = llrm_codegen::support::debug::run_main(|| llrm_driver::llc::main(&argv));
    llrm_codegen::support::debug::report_checks(&[("STALE", llrm_mir::passes::stale_checks_run())]);
    std::process::exit(status);
}
