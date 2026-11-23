//! `LLRM_MIR_PRINT_PRECOAL=<fn>` and `LLRM_COALESCE_LOG=1` were not there: the
//! MIR the coalescer is given (lenient: copies and phis are not yet MIR the pin
//! reads), and each join it tried, could not be read, so a copy left in the
//! code had no stated cause.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn the_coalescer_prints_its_input_and_each_join_it_tries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let source = tmp.join(format!("coal-{}.c", std::process::id()));
    std::fs::write(&source, "int g(int); int f(int a, int b) { int c = g(a); return c + g(b) + a; }").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .env("LLRM_MIR_PRINT_PRECOAL", "f")
        .env("LLRM_COALESCE_LOG", "1")
        .env("LLRM_MIR_PRINT_LENIENT", "1")
        .args(["-O2", "-w", "-c", "-o"])
        .arg(tmp.join(format!("coal-{}.o", std::process::id())))
        .arg(&source)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(err.contains("name: f") || err.contains("bb.0"), "no MIR of `f` before the coalescer:\n{err}");
    assert!(err.lines().any(|line| line.starts_with("COAL\t")), "no join was logged:\n{err}");
}
