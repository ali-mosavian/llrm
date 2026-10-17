//! Compile C to an object.

// `system-alloc`: tools/measure.py confirms a step rise with this build, as
// the allocator's layout moves a step's count by itself.
#[cfg(not(feature = "system-alloc"))]
#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(why) = llrm_mir::dumpdir::refuse(&argv) {
        eprintln!("llrm-c: {why}");
        std::process::exit(2);
    }
    if llrm_core::support::debug::enabled("counts") {
        llrm_mir::counts::per_pass();
    }
    let code = llrm_core::support::debug::run_main(|| llrm_c::compile::main(&argv));
    if llrm_core::support::debug::enabled("counts") {
        llrm_mir::counts::lines().iter().for_each(|line| eprintln!("{line}"));
    }
    std::process::exit(code);
}
