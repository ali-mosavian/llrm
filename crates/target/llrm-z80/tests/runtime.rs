//! The runtime (`runtime/z80_rt.s`) assembled with sdasz80 and run on
//! ucsim_z80: every routine against what C computes. A routine that is wrong
//! for some input (`__modhi3` returned `-(a % b)` for a positive dividend: its
//! sign test read stale flags) fails here with the input, not in a corpus row.

use std::process::Command;

fn have(tool: &str) -> bool {
    Command::new(tool).arg("-v").output().is_ok()
        || Command::new("which").arg(tool).output().is_ok_and(|o| o.status.success())
}

#[test]
fn the_runtime_computes_what_c_computes() {
    let missing: Vec<_> = ["sdasz80", "sdldz80", "ucsim_z80"].into_iter().filter(|tool| !have(tool)).collect();
    if !missing.is_empty() {
        let optional = std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1");
        assert!(optional, "{missing:?} not found: the runtime cannot be checked (Z80_ORACLE_OPTIONAL=1 skips it)");
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/runtime/check.py"), &format!("{dir}/runtime/z80_rt.s")])
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success() && text.contains("ALL OK"), "{text}\n{}", String::from_utf8_lossy(&done.stderr));
}
