//! -Os, two sites of one private callee: the size bound priced `grow_room` at
//! `init` as a copy that leaves the body in place, because the other call is in
//! `text`, which is met later. That call is inlined on its own verdict, so
//! `init`'s is the last call and the body goes with it
//! (gcc's `estimate_growth`). The bound refused `init` (cost 65, threshold 65)
//! and QuickrBasic `comp.c` m32 kept the callee: +92 B. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn remarks(source: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-os-two-site-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .env("LLRM_INLINE_REMARKS", "1")
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(dir.join("a.o"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    String::from_utf8_lossy(&run.stderr).into_owned()
}

#[test]
fn the_first_of_two_sites_is_priced_as_the_last_call_when_the_other_inlines() {
    let remarks = remarks("m32_os_two_site_callee.c");
    assert!(
        !remarks.lines().any(|line| line.contains("'grow_room' not inlined into 'init'")),
        "init's call was refused with the callee kept for text's:\n{remarks}"
    );
}

#[test]
fn two_sites_that_each_grow_the_code_stay_refused() {
    let remarks = remarks("m32_os_two_sites_neither_folds.c");
    for caller in ["init_a", "init_b"] {
        assert!(
            remarks.lines().any(|line| line.contains(&format!("'grow_room' not inlined into '{caller}'"))),
            "{caller}'s call was priced as a last call that another refused call keeps alive:\n{remarks}"
        );
    }
}
