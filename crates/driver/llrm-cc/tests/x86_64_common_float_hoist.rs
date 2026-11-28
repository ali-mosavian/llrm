//! x86-64 -O1: the two arms of `window_screen ? sr(e, wymin) : -sr(e, wymax)`,
//! each `movsd xmm3, [offset]` then the same arithmetic on `xmm3`, were hoisted
//! above the branch (the lane model names integer registers only, so SSE
//! operands looked independent) and read `xmm3` before either load: QB `PMAP(0,
//! 1)` printed 2 for 199, and `-201` for 0. The program prints what gcc prints.
//! Needs `LLRM_LLC` and a host `cc` for the link.

use std::path::Path;
use std::process::Command;

#[test]
fn two_arms_each_loading_the_offset_they_read_are_not_hoisted_above_the_branch() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("float-hoist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c");
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-m64", "-fno-pic", "-O1", "-w", "-c", "-o"])
        .arg(&object)
        .arg(tests.join("float_offset_select_branch.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let exe = dir.join("a.out");
    let link = Command::new("cc").args(["-static", "-no-pie", "-o"]).arg(&exe).arg(&object).output().unwrap();
    assert!(link.status.success(), "{}", String::from_utf8_lossy(&link.stderr));
    let out = Command::new(&exe).output().unwrap();
    let expected = std::fs::read_to_string(tests.join("float_offset_select_branch.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}
