//! x86-32 -Os: bytes decide the keep-hinted-calls trial. On `x_sf_add` the kept
//! calls to `sf_shr_sticky32` and `unpack` are 35 B smaller (1,528 against
//! 1,563) and +3.7% clk (78,166 against 75,410), the clocks the counted wrapped
//! cell already runs and under gcc's 81,200. The trial keeps the calls at up to
//! a clock a byte saved (here 0.1). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_hinted_callee_stays_a_call_where_the_calls_cost_four_percent_clocks() {
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
    assert!(text.contains("call sf_shr_sticky32"), "the hinted callee was inlined, 35 B larger:\n{text}");
}
