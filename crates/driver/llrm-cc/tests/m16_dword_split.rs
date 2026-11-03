//! A dword read only as its two words is two word loads (DEVIATE LS1), where
//! the pin keeps one dword load and `shld`. Needs the pinned clang
//! (`LLRM_CLANG_M16`) and the Open Watcom headers (`LLRM_OW`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn has(
    code: &[u8],
    pattern: &[u8],
) -> bool {
    code.windows(pattern.len()).any(|window| window == pattern)
}

fn compile(
    name: &str,
    source: &str,
) -> Vec<u8> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16split-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-c", "-march=i386", "-o"])
        .arg(dir.join("d.o"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read(dir.join("d.o")).unwrap()
}

fn available() -> bool {
    let set = std::env::var_os("LLRM_CLANG_M16").is_some() && std::env::var_os("LLRM_OW").is_some();
    if !set {
        eprintln!("skipped: LLRM_CLANG_M16 or LLRM_OW is not set");
    }
    set
}

/// `long g(long *p) { return *p; }` was `mov eax,[bx]; shld edx,eax,16` (8 B,
/// 3 clk); `mov ax,[bx]; mov dx,[bx+2]` is 5 B, 2 clk.
#[test]
fn a_dword_returned_in_dx_ax_is_two_word_loads() {
    if !available() {
        return;
    }
    let code = compile("ret", "long g(long *p) { return *p; }\n");
    assert!(has(&code, &[0x8b, 0x57, 0x02]), "no `mov dx,[bx+2]`");
    assert!(!has(&code, &[0x66, 0x0f, 0xa4]), "a `shld` is left");
}

/// A use of the whole dword keeps the one dword load.
#[test]
fn a_dword_that_is_also_stored_whole_keeps_its_load() {
    if !available() {
        return;
    }
    let code = compile("whole", "short h(long *p, long *q) { *q = *p; return (short)*p; }\n");
    assert!(has(&code, &[0x66, 0x8b]), "the dword load is gone");
}
