//! Compile QuickBASIC to an object.

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv == ["--build-info"] {
        println!("{}", llrm_mir::buildinfo::MARK);
        return;
    }
    if let Err(why) = llrm_mir::dumpdir::refuse(&argv) {
        eprintln!("llrm-qb: {why}");
        std::process::exit(2);
    }
    let code = llrm_core::support::debug::run_main(|| llrm_qb::cli::main(&argv));
    std::process::exit(code);
}
