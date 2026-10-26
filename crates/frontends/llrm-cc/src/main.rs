//! `llrm-cc [cc options] FILE...`
fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(llrm_cc::run_main(&argv));
}
