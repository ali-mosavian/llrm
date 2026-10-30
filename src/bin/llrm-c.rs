//! Compile C to an object.

// `system-alloc`: tools/measure.py confirms a step rise with this build, as
// the allocator's layout moves a step's count by itself.
#[cfg(not(feature = "system-alloc"))]
#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if llrm_codegen::support::version::asked(
        &argv,
        "llrm-c",
        env!("LLRM_BUILD_SHA"),
        env!("LLRM_BUILD_DIRTY"),
        env!("LLRM_BUILD_INCREMENTAL"),
    ) {
        return;
    }
    if let Err(why) = llrm_mir::dumpdir::refuse(&argv) {
        eprintln!("llrm-c: {why}");
        std::process::exit(2);
    }
    if llrm_codegen::support::debug::enabled("counts") {
        llrm_mir::counts::per_pass();
    }
    let code = llrm_codegen::support::debug::run_main(|| llrm_c::compile::main(&argv));
    llrm_codegen::support::debug::report_checks(&[
        ("STALE", llrm_mir::passes::stale_checks_run()),
        ("REPLAY", llrm_mir::passes::replay_checks_run()),
    ]);
    if llrm_codegen::support::debug::enabled("counts") {
        llrm_mir::counts::lines().iter().for_each(|line| eprintln!("{line}"));
        llrm_transforms::parity::tally_lines().iter().for_each(|line| eprintln!("{line}"));
    }
    std::process::exit(code);
}
