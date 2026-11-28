//! x86-64 -O2: the zero a loop unpacks against (`V_SET0`, `xorps r, r`) is
//! made once before the loop, as the pin's EarlyMachineLICM hoists every
//! trivially rematerializable instruction. MachineLICM took no xmm value on
//! that path, so `widen` zeroed a register on every trip, and with the zero
//! exported to a phi (WSZ-CG) mem_length_boundaries_m32 spilled its loop
//! accumulators (+86 B). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(source: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("zero-hoist-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// The lines from the label a backward `jne` names to that `jne`.
fn loop_body(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let jump = lines.iter().position(|line| line.starts_with("jne ")).expect("a loop");
    let target = format!("{}:", lines[jump].trim_start_matches("jne ").trim());
    let start = lines[..jump].iter().rposition(|line| *line == target).expect("a backward jump");
    lines[start..jump].to_vec()
}

#[test]
fn the_zero_a_loop_unpacks_against_is_made_before_it() {
    let text = assembly("widen_bytes.c");
    let body = loop_body(&text);
    assert!(body.iter().any(|line| line.starts_with("punpcklbw")), "premise: the loop unpacks: {body:?}");
    assert!(!body.iter().any(|line| line.starts_with("xorps")), "zeroed on every trip: {body:?}");
}
