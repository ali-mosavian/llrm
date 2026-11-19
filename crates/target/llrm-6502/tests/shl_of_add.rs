//! `t[3 * at + (a < b ? 1 : 2)]` over words. Folding the shift into the add
//! (shl (add x, c), n -> add (shl x, n), c << n) ran the shift on the
//! zero-page cells (`asl; rol`) where the add had gone through Y and X:
//! bintree -O2 +20 B. Needs `llrm-cc` built (`LLRM_CC`) and cc65.

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

#[test]
fn a_shifted_sum_with_a_small_constant_is_not_commuted() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shl-of-add");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shl_commute.c");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", "-O2", "-S", "-w", "-o", "o.s"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let built = Command::new("ca65").current_dir(&dir).args(["-U", "-o", "o.o", "o.s"]).output().unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let dump = Command::new("od65").current_dir(&dir).args(["--dump-segsize", "o.o"]).output().unwrap();
    let text = String::from_utf8_lossy(&dump.stdout);
    let code: u32 = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("CODE:").and_then(|n| n.trim().parse().ok()))
        .unwrap_or_else(|| panic!("no CODE size in {text}"));
    assert!(code <= 157, "CODE {code} B; the unfolded form is 157 B, the folded 161");
}
