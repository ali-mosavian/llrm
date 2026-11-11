//! `LLRM_CHECK_MODULES` panicked ("declarations: brought up to date, it is not
//! what working it out afresh gives ... @fib kept ... afresh ...
//! Flag(\"tre-deferred\")") where the walk marked a tree recursion's tail-call
//! elimination as deferred without telling the analyses the body changed
//! (`bench/frames/frames.c`, 1 of 166 programs; x86_32 and x86_64, -O2 and
//! -O3). Needs the Open Watcom tree (tools/ow-fetch.sh) and `LLRM_PIN_CLANG`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "static int fib(int n) { if (n < 2) return n; return fib(n - 1) + fib(n - 2); }\nstatic int d(int v) { int t = 0; while (v > 0) { t += 1; v--; } return t; }\nint g(int i) { return d(i); }\nint h(int j) { return fib(j); }\n";

#[test]
fn a_function_the_walk_flags_for_deferred_tail_recursion_elimination_is_told_changed() {
    assert!(
        llrm_cc::ow_present(),
        "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip"
    );
    assert!(
        std::env::var_os("LLRM_PIN_CLANG").is_some(),
        "LLRM_PIN_CLANG is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("checkmodules-walk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_CHECK_MODULES", "1")
        .args(["-O2", "-w", "-c", "-o"])
        .arg(dir.join("f.o"))
        .arg(dir.join("f.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}
