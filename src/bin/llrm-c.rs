//! Compile C to an object.

// `system-alloc`: tools/measure.py confirms a step rise with this build, as
// mimalloc's heap layout moves a step's count by itself.
#[cfg(not(feature = "system-alloc"))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(why) = llrm_mir::dumpdir::refuse(&argv) {
        eprintln!("llrm-c: {why}");
        std::process::exit(2);
    }
    if llrm_core::support::debug::enabled("counts") {
        llrm_mir::counts::per_pass();
    }
    // The source is the last argument that is no option and no option's value.
    let source = argv
        .iter()
        .enumerate()
        .filter(|(at, one)| {
            !one.starts_with('-') && (*at == 0 || !matches!(argv[*at - 1].as_str(), "-o" | "-I" | "--dump"))
        })
        .map(|(_, one)| one.as_str())
        .next_back()
        .unwrap_or_default();
    let level = argv.iter().rfind(|one| one.starts_with("-O")).map_or("-O2", String::as_str);
    llrm_mir::trace::count_work_with(llrm_core::support::debug::work);
    llrm_mir::trace::start(&format!("{source}@{}", &level[1..]));
    let code = llrm_core::support::debug::run_main(|| llrm_c::compile::main(&argv));
    llrm_mir::trace::flush();
    if llrm_core::support::debug::enabled("counts") {
        llrm_mir::counts::lines().iter().for_each(|line| eprintln!("{line}"));
    }
    std::process::exit(code);
}
