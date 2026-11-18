//! x86-64 -O2: `i16 * i16` widened to i32 in a loop. Without
//! `reduceVMULWidth`/`combineMulToPMADDWD` the `mul <4 x i32>` was scalarised
//! (`pmuludq` and shuffles, SSE2 has no `pmulld`), and the repack of
//! `pmulhuw`/`pmullw` went through the stack (no shuffle was `punpcklwd`).
//! Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-widen-multiply-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/vector_widen_multiply.c"))
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
fn signed_words_multiply_with_pmaddwd() {
    let text = assembly();
    let words = body(&text, "words");
    assert!(words.contains("pmaddwd") && !words.contains("pmuludq"), "{words}");
}

#[test]
fn unsigned_words_multiply_with_pmullw_pmulhuw_and_unpack() {
    let text = assembly();
    let words = body(&text, "unsigned_words");
    assert!(words.contains("pmullw") && words.contains("pmulhuw") && words.contains("punpcklwd"), "{words}");
    assert!(!words.contains("pmuludq") && !words.contains("[rsp"), "{words}");
}
