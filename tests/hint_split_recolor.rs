//! A hinted vreg that missed its register is split or recolored, not copied.

use std::process::Command;

/// `place` of queens at m32 -O2 (dag). `trySplitAroundHintReg` and
/// `tryHintsRecoloring` move a vreg onto the register its copies want. Without
/// them the loop kept six register-to-register copies; the pin keeps three.
#[test]
fn a_missed_hint_leaves_at_most_three_register_copies_in_queens_place() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/queens_place_arg.ll");
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("x.s");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-m32", "-O2", "-march=i486", "--isel=dag", "-S", "-o"])
        .arg(&out)
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(&out).unwrap();
    let copies = text
        .lines()
        .skip_while(|line| !line.starts_with("_place_ proc"))
        .take_while(|line| !line.starts_with("_place_ endp"))
        .filter(|line| {
            let operands = line.trim().strip_prefix("mov ").unwrap_or("");
            operands.split_once(", ").is_some_and(|(to, from)| to.len() == 3 && from.len() == 3)
        })
        .count();
    assert!(copies <= 3, "{copies} register copies\n{text}");
}
