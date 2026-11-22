//! The pin runs `MemCpyOpt` once, before the sink and the phi folds shape a
//! compound literal stored into an array element as `memset(tmp)`;
//! `memcpy(dst, tmp)`. Run again at size, the copy is a `memset` of the
//! destination and the 70-byte temporary is dead: QuickrBasic `dialog.c`
//! `bind` was 391 B against gcc's 309 B (a `rep movs` out of a stack
//! temporary). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-os-late-memcpyopt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_late_memcpyopt.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn a_compound_literal_is_built_in_place_at_os() {
    let text = listing();
    let copies = text.lines().filter(|line| line.contains("rep movs")).count();
    assert_eq!(copies, 0, "the literal is copied out of a stack temporary\n{text}");
}

