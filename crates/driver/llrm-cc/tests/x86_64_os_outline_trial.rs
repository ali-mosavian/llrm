//! `static inline name_at` (an arena base, a table offset, a scaled index) was
//! inlined at all seven of its calls at -Os: the units count 2 against a call
//! of 5, the object writes ~11 B a copy and came out 52 B (174 vs 122 B)
//! larger than the one copy gcc keeps (QB bind.c `symb_name`). The finished
//! object decides (OS-OUTLINE-TRIAL). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing(off: bool) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-outline-trial-{}-{off}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let mut run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"));
    run.env("LLRM_CC_TARGET", "x86_64-linux").env("LLRM_LLC", llc);
    if off {
        run.env("LLRM_NO_OUTLINE_TRIAL", "1");
    }
    let run = run
        .args(["-Os", "-w", "-ffreestanding", "-fno-pic", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m64_os_outline_helper.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

fn calls(text: &str) -> usize {
    text.lines().filter(|line| line.contains("call") && line.contains("name_at")).count()
}

#[test]
fn a_helper_whose_copies_grow_the_object_stays_a_call_at_os() {
    let text = listing(false);
    assert_eq!(calls(&text), 7, "the helper is inlined at its seven calls (174 B, one copy 122 B):\n{text}");
}

#[test]
fn the_trial_is_the_only_thing_that_keeps_them() {
    let text = listing(true);
    assert_eq!(calls(&text), 0, "without the trial the size bound inlines every copy:\n{text}");
}
