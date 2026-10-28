//! The range checks of EXP, `^` and LOG (B$CHKEXP, B$CHKEX2, B$CHKLOG) on a
//! host, at their edges: runtime/qb/posix/mathops.c, tests/qbhost/chk_unit.c.

use std::path::Path;
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A check that errors on the wrong side of its bound lets EXP(22712.8) or
/// LOG(0) through as INF or IND instead of ERROR 6 or 5.
#[test]
fn the_host_range_checks_hold_at_their_edges() {
    let qb = root().join("runtime/qb");
    let out = std::env::temp_dir().join(format!("chk_unit_{}", std::process::id()));
    let built = Command::new("cc")
        .args(["-std=gnu11", "-Wall", "-Wno-unknown-pragmas", "-I"])
        .arg(&qb)
        .arg("-I")
        .arg(qb.join("posix"))
        .arg(root().join("tests/qbhost/chk_unit.c"))
        .arg(qb.join("posix/mathops.c"))
        .arg("-lm")
        .arg("-o")
        .arg(&out)
        .output()
        .expect("cc runs");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let done = Command::new(&out).output().expect("the test runs");
    std::fs::remove_file(&out).ok();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stdout));
}
