//! x86-32 -Os: the trial work model priced a call and a sibling call as one
//! instruction and a tail-called callee as no work, so `KeepHintedCalls` read
//! `x_sf_add` as saving 125 work (+3.6% clk in truth, 78,153 against 75,413) and
//! kept `sf_shr_sticky32` and `unpack` out of line. Call, return and sibling
//! call now carry their clocks and the callee's weight. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_hinted_callee_stays_inlined_where_the_calls_cost_four_percent_clocks() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-call-clocks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/llrm-x86-m32/vsgcc/kernels/x_sf_add/x_sf_add.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(!text.contains("call sf_shr_sticky32"), "the hinted callee was kept as a call:\n{text}");
}
