//! The clang front end driver, built beside `llrm-llc` so tests that spawn it
//! find both.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(why) = llrm_mir::dumpdir::refuse(&argv) {
        eprintln!("llrm-cc: {why}");
        std::process::exit(2);
    }
    std::process::exit(llrm_cc::run_main(&argv));
}
