//! The 6502 cost model (`llrm_6502::tti`) reaches the passes that price code.

mod common;

use common::pipeline::datalayout;

/// The mid-end's last stage of `ir` at -O2, as `llrm-6502-llc` dumps it.
fn final_ir(ir: &str) -> String {
    let dir = std::env::temp_dir().join(format!("llrm6502-tti-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("f.ll");
    std::fs::write(&source, format!("target datalayout = \"{}\"\n{ir}\n", datalayout())).unwrap();
    let ran = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
        .args(["--platform=sim65", "-S", "-O2", "-o", dir.join("f.s").to_str().unwrap(), source.to_str().unwrap()])
        .env("LLRM_MIR_STAGES", dir.join("stages"))
        .status()
        .unwrap();
    assert!(ran.success());
    let mut stages: Vec<_> = std::fs::read_dir(dir.join("stages"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
        .collect();
    stages.sort();
    let text = std::fs::read_to_string(stages.last().unwrap()).unwrap();
    std::fs::remove_dir_all(dir).ok();
    text
}

/// With every operation priced 1 whatever its width, `SimplifyCFG` ran the arm of `if (c) r = a == 0 ? 3 : 0`
/// on both paths: a 32-bit select (four byte selects) for the sake of one branch. `strnicmp` did that
/// to its tail and came out 247 bytes against 147.
#[test]
fn a_long_select_is_not_speculated_to_save_a_branch() {
    let text = final_ir(
        "define i32 @f(i1 %c, i8 %a) {
  br i1 %c, label %t, label %m
t:
  %x = icmp eq i8 %a, 0
  %s = select i1 %x, i32 3, i32 0
  br label %m
m:
  %r = phi i32 [ 1, %0 ], [ %s, %t ]
  ret i32 %r
}",
    );
    assert!(text.contains("br i1 %c"), "the arm was run on both paths:\n{text}");
}
