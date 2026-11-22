//! A recursion `deep(n - 1, b, a + n) + n` called once with constants stayed a
//! function (58 B in arg_registers_abi `-Os`): IPSCCP saw the self-call's
//! varying arguments. gcc turns the tail recursion into a loop before ipa-cp
//! (`pass_tail_recursion`), so every caller passes the same constants and the
//! loop folds to one `mov`. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-early-tre-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/early_tail_recursion.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn a_recursion_called_with_constants_folds_away() {
    let text = listing();
    assert!(!text.contains("<deep>:"), "deep stayed a function:\n{text}");
}
