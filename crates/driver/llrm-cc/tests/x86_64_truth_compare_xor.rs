//! `early_exit() != want` of two 0/1 values was `xor ecx,ecx; cmp; setne cl`
//! (the pin keeps the compare); gcc's `simplify_truth_ops_using_ranges` makes
//! it one `xor`. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-truth-xor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/truth_compare_xor.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout)
        .lines()
        .skip_while(|line| !line.contains("<differs_from_expected>:"))
        .skip(1)
        .take_while(|line| line.contains(":\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_not_equal_of_two_truth_values_is_one_xor() {
    let text = listing();
    assert!(!text.contains("setne"), "the compare stayed:\n{text}");
    assert!(
        text.lines()
            .any(|line| line.contains("xor ") && !line.contains("xor    eax,eax") && !line.contains("xor    ecx,ecx")),
        "{text}"
    );
}
