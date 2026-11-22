//! A cell the 65C02 zeroed with `stz` is known to hold 0, as one zeroed by
//! `lda #0; sta` is: a reload of it while `a` holds 0 goes. `rep_count` -O1 kept
//! two zero cells (`stz rc2; stz rc3`) and read one back before each of 16 zero
//! stores through a pointer: 300 B on the 65C02 against 269 B on the 6502
//! (llvm-mos 294). Needs `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(target: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("zero-cell-reload-{target}"));
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/rep_count.c");
    let out = dir.join("rep_count.s");
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args([&format!("--target={target}-sim65"), "-O1", "-fwhole-program", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn a_zero_stored_with_stz_is_not_reloaded_while_a_holds_zero() {
    let text = assembly("65c02");
    let at = text.find("\nmain_b3:").expect("the zeroing loop (premise)");
    let body = text[at..].split("\nmain_b12:").next().unwrap();
    assert!(body.contains("  lda #0\n"), "premise: `a` holds 0 in the loop:\n{body}");
    let reloads = body.lines().filter(|line| matches!(line.trim(), "lda rc2" | "lda rc3")).count();
    assert_eq!(reloads, 0, "a zero cell is reloaded:\n{body}");
}
