//! Placement saw a return block as one `ret`: no teardown, so under the
//! two-instruction copy bound, and it copied the block into the arms that
//! reach it. The pin places after the epilogue is in (four pops and a `ret`
//! in sfm16's `add_bits`), too long to copy, and keeps one return at the end.
//! sfm16_float_add -O2 -m32 -march=i486: 3660 B of code, 3556 B with the
//! teardown counted.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_o2_a_return_with_a_frame_teardown_is_not_copied_by_placement() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("framed-ret-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source = root.join("bench/sfm16_float_add/sfm16_float_add.c");
    let object = dir.join("c.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", "-O2", "-w", "-c", "-o"])
        .arg(&object)
        .arg("-I")
        .arg(root.join("runtime/shared/posix"))
        .arg("-I")
        .arg(root.join("bench"))
        .arg("-I")
        .arg(root.join("runtime/compiler-rt/lib/fp"))
        .arg(source)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let sizes = Command::new("size").arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&sizes.stdout);
    let code: u32 = text.lines().nth(1).and_then(|row| row.split_whitespace().next()).unwrap().parse().unwrap();
    assert!(code <= 3600, "{code} B of code, 3556 expected, 3660 before the teardown was counted:\n{text}");
}
