//! x86-64 `long double` compares read the status word (`fcom; fnstsw ax; sahf`)
//! and a `?:` on one branched: the pin sets the flags with `fucomi` and selects
//! with `fcmov`. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_long_double_compare_sets_the_flags_and_a_select_is_an_fcmov() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("long-double-compare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/long_double_compare.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(text.contains("fucomi"), "no fucomi:\n{text}");
    assert!(text.contains("fcmovnbe"), "the select branched:\n{text}");
    assert!(!text.contains("fnstsw") && !text.contains("sahf"), "the status word was read:\n{text}");
}
