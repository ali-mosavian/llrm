//! x86-64 -O2: the zero a loop unpacks against (`V_SET0`, `xorps r, r`) is
//! made once before the loop, as the pin's EarlyMachineLICM hoists every
//! trivially rematerializable instruction. MachineLICM took no xmm value on
//! that path, so `widen` zeroed a register on every trip, and with the zero
//! exported to a phi (WSZ-CG) mem_length_boundaries_m32 spilled its loop
//! accumulators (+86 B).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn assembly(
    source: &Path,
    level: &str,
) -> String {
    let llc = target_dir::llc(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."));
    // Tests run in parallel: a directory of its own for each source.
    let stem = source.file_stem().and_then(|stem| stem.to_str()).unwrap_or("source");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("zero-hoist-{}-{stem}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(source)
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
    let text = assembly(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/widen_bytes.c"), "-O2");
    let body = loop_body(&text);
    assert!(body.iter().any(|line| line.starts_with("punpcklbw")), "premise: the loop unpacks: {body:?}");
    assert!(!body.iter().any(|line| line.starts_with("xorps")), "zeroed on every trip: {body:?}");
}

/// A hoisted zero that must live across a call is made again after it
/// (`remat_shape` has the zero idiom). It was spilled and reloaded instead:
/// pattern_fill -O1 `main` +3 B, int64_divmod_edges -Os +20 B.
#[test]
fn a_hoisted_zero_is_made_again_after_a_call_not_spilled() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/pattern_fill.c");
    let text = assembly(&source, "-O1");
    assert!(text.contains("xorps") && text.contains("call"), "premise: a zero and a call");
    let spills: Vec<&str> = text.lines().filter(|line| line.contains("xmmword ptr [rsp")).collect();
    assert!(spills.is_empty(), "a zero spilled: {spills:?}");
}
