//! `shr rax, 32` wrote no flag in the effects table (the count was masked with
//! 31 at every width; a 64-bit shift masks with 63), so the two-address pass
//! moved it between a `test` and its `jne` and the branch took the wrong arm:
//! QuickrBasic's `FOR i# = 1 TO 2` compiled as a single loop at -O2. Needs
//! `LLRM_LLC` and a host `cc`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_shift_by_32_of_a_64_bit_value_leaves_the_flags_of_the_test_before_its_branch_alone() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("shift-flags-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c");
    let ours = dir.join("f.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-m64", "-fno-pic", "-O2", "-w", "-c", "-o"])
        .arg(&ours)
        .arg(tests.join("shift32_flags_f.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let exe = dir.join("a.out");
    let link = Command::new("cc")
        .args(["-static", "-no-pie", "-o"])
        .arg(&exe)
        .arg(&ours)
        .arg(tests.join("shift32_flags_main.c"))
        .output()
        .unwrap();
    assert!(link.status.success(), "{}", String::from_utf8_lossy(&link.stderr));
    let out = Command::new(&exe).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "op 166 1\nop2 166\nop3 5 7\n");
}
