//! x86-32 -O3: a loop whose body holds fewer values than the registers need
//! (7 against 6, short of LSR-SPILL-PRICE's margin of two) keeps its pointer
//! induction variable: sfm16_float_cmp ran 22,268 clk with the step and 23,068
//! when the spill pricing dropped it (two `lea` and a scaled `fld` a trip).
//! Needs the Open Watcom tree (tools/ow-fetch.sh) and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_loop_short_of_the_spill_margin_keeps_its_pointer_step() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lsr-unsaturated-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", llc)
        .args(["-O3", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/lsr_unsaturated_loop.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let steps = asm.lines().filter(|line| line.trim().starts_with("add ") && line.trim().ends_with(", 44")).count();
    assert_eq!(steps, 1, "the loop lost its pointer step by 44:\n{asm}");
}
