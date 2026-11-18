//! x86-32 -Os (i486): a constant copy of three or more dwords is `rep movs`,
//! as gcc's `MOVE_RATIO` at size; the pin keeps 4 pairs (three load/store
//! pairs for 12 bytes). A fill of up to eight stores stays stores (a 486
//! `rep stos` is 7 + 4 clk a dword: lru -Os +24% clocks). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("rep-string-size-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/rep_string_size.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body")
}

#[test]
fn a_seven_dword_clear_stays_stores_for_the_clocks() {
    let text = assembly();
    let clear = body(&text, "clear");
    assert!(!clear.contains("rep stos"), "{clear}");
}

#[test]
fn a_three_dword_copy_is_rep_movs() {
    let text = assembly();
    let copy = body(&text, "copy3");
    assert!(copy.contains("rep movsd"), "{copy}");
}
