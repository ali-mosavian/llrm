//! A callee emptied by the write-only removal of its only store keeps the `llrm.reads` it was
//! stamped with, so the calls of it stay (byvalue_struct_address -Os: `foo` 40 B against clang's
//! 1; here `forward` is a `jmp` to an empty `add_row`). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-os-empty-callee-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/os_empty_callee.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn the_call_of_a_callee_the_write_only_removal_emptied_is_gone_at_os() {
    let text = listing();
    let forward: Vec<_> = text
        .lines()
        .skip_while(|line| !line.contains("<forward>:"))
        .skip(1)
        .take_while(|line| line.contains(":\t"))
        .collect();
    assert_eq!(forward.len(), 1, "forward is a `ret`, not a call of the empty callee:\n{text}");
    assert!(forward[0].contains("ret"), "{text}");
}
