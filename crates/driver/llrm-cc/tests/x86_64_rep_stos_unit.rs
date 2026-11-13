//! x86-64 -Os: a constant fill is the `rep stos` unit with the fewest bytes
//! (DEVIATE REP-STOS-UNIT). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(source: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("rep-stos-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// A 700-byte frame object of 0xEE was `movabs rax, 0xEEEE..; mov ecx, 87; rep
/// stosq; mov dword [m], 0xEEEEEEEE` (28 B) where `mov al, -18; mov ecx, 700;
/// rep stosb` is 9 B (memory_expansion -Os, 31 functions of +15..23 B).
#[test]
fn a_frame_object_fill_at_size_is_a_rep_stosb_when_that_is_fewer_bytes() {
    let text = assembly("rep_stos_frame.c");
    assert!(text.contains("rep stosb"), "no rep stosb:\n{text}");
    assert!(!text.contains("movabs") && !text.contains("stosq"), "the quadword form:\n{text}");
}
