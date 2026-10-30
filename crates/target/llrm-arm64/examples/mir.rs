//! `cargo run -p llrm-arm64 --example mir -- FILE.ll`: the module after the
//! shared mid-end, as the arm64 selector sees it.
fn main() {
    let path = std::env::args().nth(1).expect("FILE.ll");
    let text = std::fs::read_to_string(&path).expect("readable");
    let (triple, _) = llrm_arm64::driver::without_triple(&text);
    let platform =
        triple.as_deref().and_then(llrm_arm64::driver::platform_of).unwrap_or(llrm_arm64::machine::Platform::Darwin);
    let mut program = llrm_arm64::driver::program(&text, platform).expect("reads");
    llrm_arm64::driver::optimized(&mut program).expect("optimizes");
    print!("{}", llrm_mir::print::module(&program.modules[0]));
}
