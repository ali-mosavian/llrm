//! `static inline sf_clz32` (clang's `inlinehint`) of QuickrBasic's `sfm16_dbg`
//! was inlined at all six of its calls at -Os, 388 B larger than the one copy
//! gcc keeps: the pin's hint threshold lets the callee in whatever it costs,
//! and the units cannot say what the copies fold to. The finished object
//! decides (INLINEHINT-GROWTH-TRIAL). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-inlinehint-growth-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_inlinehint_growth.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn a_hinted_callee_that_grows_the_object_stays_one_copy_at_os() {
    let text = listing();
    let calls = text.lines().filter(|line| line.contains("call") && line.contains("lead_zeros")).count();
    assert!(
        text.contains("<lead_zeros>:") && calls >= 5,
        "the callee is inlined at its six calls (calls to it: {calls})\n{text}"
    );
}
