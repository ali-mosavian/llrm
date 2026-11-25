//! -Os, two sites of one private callee: the size bound priced `grow_room` at
//! `init` as a copy that leaves the body in place, because the other call is in
//! `text`, which is met later. That call is inlined on its own verdict, so
//! `init`'s is the last call and the body goes with it
//! (gcc's `estimate_growth`). The bound refused `init` (cost 65, threshold 65)
//! and QuickrBasic `comp.c` m32 kept the callee: +92 B. Needs `LLRM_LLC`.
//!
//! These read the finished object. The remarks of a compile include the size
//! trials' own rounds, where a call is kept on purpose (`init`: cost 65,
//! threshold 65), so a remark names no outcome.

use std::path::Path;
use std::process::Command;

/// The symbols of `source` compiled at -Os with `flags`.
fn symbols(
    source: &str,
    flags: &[&str],
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-os-two-site-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-ffreestanding"])
        .args(flags)
        .args(["-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let symbols = Command::new("nm").arg(&object).output().unwrap();
    String::from_utf8_lossy(&symbols.stdout).into_owned()
}

#[test]
fn the_first_of_two_sites_is_priced_as_the_last_call_when_the_other_inlines() {
    let symbols = symbols("m32_os_two_site_callee.c", &[]);
    assert!(!symbols.contains("grow_room"), "the callee was kept for text's call, init's included:\n{symbols}");
}

/// With the callee's `inline` hint (clang's `inlinehint`, which the pin lets
/// in at threshold 325) the finished object inlines both sites, 3 B larger:
/// the trial that keeps the calls (-3 B for +44 clocks) is refused. The size
/// bound's own verdict is the unhinted callee's: neither site folds, each
/// grows the code, and the callee stays with both calls (`-Dinline=` takes the
/// hint off).
#[test]
fn two_sites_that_each_grow_the_code_stay_refused() {
    let symbols = symbols("m32_os_two_sites_neither_folds.c", &["-Dinline="]);
    assert!(symbols.contains("grow_room"), "both calls were inlined, the callee priced as a last call:\n{symbols}");
}
