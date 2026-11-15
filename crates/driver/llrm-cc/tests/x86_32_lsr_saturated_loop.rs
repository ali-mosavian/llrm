//! x86-32 -O3: a loop whose body already fills the registers keeps no pointer
//! induction variables (DEVIATE LSR-SPILL-PRICE). The soft-float add loop
//! stepped two pointers by 44 beside its counter and index, every one held in
//! a stack slot: x_sf_sub 42,196 clk against 38,807 without them (gcc 35,654).
//! Needs the Open Watcom tree (tools/ow-fetch.sh) and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_loop_with_no_free_register_steps_no_pointer_by_the_row_size() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lsr-saturated-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", llc)
        .args(["-O3", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/lsr_saturated_loop.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let steps = asm.lines().filter(|line| line.trim().ends_with(", 44") && line.trim().starts_with("add ")).count();
    assert_eq!(steps, 0, "the loop steps {steps} pointers by 44:\n{asm}");
}
