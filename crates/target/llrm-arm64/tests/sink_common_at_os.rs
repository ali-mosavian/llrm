//! DEVIATE M32-SINK-COMMON-SIZE on arm64 (gcc `-fcrossjumping`): a function
//! optimized for size sinks the code the arms of a branch share in every
//! SimplifyCFG slot (the pin: the late one). Three inlined copies of one loop
//! kept a body each: QB `synth.c`'s 20 `voice_run` copies were 26,456 B, and
//! 9,896 B sunk; the reduction here is 110 instructions, 52 sunk.

use std::process::Command;

fn instructions(
    ir: &str,
    level: &str,
) -> usize {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, ir).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", level, "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(asm.contains("voice:"), "premise: lowered:\n{asm}");
    asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count()
}

#[test]
fn a_size_function_sinks_the_loops_common_code_and_a_speed_one_does_not() {
    let size = include_str!("data/sink_common_voices.ll");
    let speed = size.replace(" optsize", "");
    assert!(instructions(size, "-Os") <= 60, "size: the three copies share their code");
    assert!(instructions(&speed, "-O2") >= 100, "premise: the pin keeps a body per copy");
}
