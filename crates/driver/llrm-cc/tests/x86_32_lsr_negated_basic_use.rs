//! x86-32 -O1: `row - r` and `r - row` in one loop share one induction
//! variable (LSR-NEG-BASIC): queens ran 162,805 clk with three registers
//! stepping (`inc`, `dec`, `add 4`) and 152,019 with two.
//! Needs the Open Watcom tree (tools/ow-fetch.sh) and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_loop_and_its_negation_share_one_induction_variable() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lsr-negated-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", llc)
        .args(["-O1", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/lsr_negated_basic_use.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let start = asm.lines().position(|line| line.trim() == "safe_ proc near").expect("safe_");
    let body: Vec<&str> =
        asm.lines().skip(start).take_while(|line| line.trim() != "safe_ endp").map(str::trim).collect();
    let steps = body
        .iter()
        .filter(|line| line.starts_with("inc ") || line.starts_with("dec ") || (line.starts_with("add ") && line.ends_with(", 4")))
        .count();
    assert_eq!(steps, 2, "the loop steps three registers:\n{}", body.join("\n"));
}
