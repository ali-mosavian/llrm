//! x86-32 -O1: the last select of a chain, `shape == 3 ? w >> 29 : w`, lays its
//! arm out as the fall-through of the compare (`cmp 3; jne join; shr`).
//! Placement saw the select's empty false arm still there, took it first and
//! left the arm out of line (`cmp 3; je arm; jmp join`), +2 B a copy:
//! div_constants_long -O1 +28 B. Needs the Open Watcom tree (tools/ow-fetch.sh)
//! and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "extern unsigned long next(void);\nunsigned long sum;\nvoid f(int n)\n{\n    int i;\n    for (i = 0; i < n; ++i) {\n        unsigned long w = next(), shape = next() & 7;\n        unsigned long x = shape == 0 ? w : shape == 1 ? w >> 16 : shape == 2 ? w >> 24 : shape == 3 ? w >> 29 : w;\n        sum += x * 31;\n    }\n}\n";

#[test]
fn the_last_arm_of_a_select_chain_falls_through_the_compare() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("select-chain-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", llc)
        .args(["-O1", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at =
        lines.iter().position(|line| line.starts_with("cmp") && line.ends_with(", 3")).expect("the compare with 3");
    assert!(lines[at + 1].starts_with("jne "), "the arm (shr 29) is out of line after `{}`:\n{text}", lines[at]);
}
