//! An argument the other blocks read is the register it arrived in, not a
//! copy of it (`LowerArguments`: "skip creating an unnecessary vreg"). Ours
//! copied it, so `row + 1` found its source still read later and was `mov` +
//! `inc` where the pin has one `lea`: queens -O1 `place` ran 512 instructions
//! more.

use std::process::Command;

const SOURCE: &str = include_str!("../bench/queens/queens.c");

#[test]
fn an_argument_the_loop_reads_is_added_to_with_one_lea_not_mov_and_inc() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O1", "-fno-inline-functions", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let from = listing.find("place proc").expect("premise: `place` is a function");
    let asm = &listing[from..from + listing[from..].find("place endp").expect("its end")];
    assert!(asm.contains("call place"), "premise: it recurses\n{asm}");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let copied_then_incremented = lines.windows(2).any(|pair| {
        let Some(register) = pair[0].strip_prefix("mov ").and_then(|rest| rest.split(',').next()) else { return false };
        pair[1] == format!("inc {register}")
    });
    assert!(!copied_then_incremented, "mov + inc where lea fits:\n{asm}");
}
