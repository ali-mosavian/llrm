//! x86-64 callee-saved registers are taken in the pin's order: rbx, rbp, r14,
//! r15, r12, r13 (`RegisterClassInfo` appends the CSR aliases in the register
//! class's order). The registers file listed r12, r13, r14, r15, so a function
//! with three values live across calls pushed rbx, r12, r13 where the pin
//! pushes rbx, r14, r15. A function that needs no frame register has rbp as
//! the second of them (`hasFP` false: rbp is reserved only when there is a
//! frame pointer), as the pin: rbx, rbp, r14. It was never given out in long
//! mode, because the frame register was named `rbp` where the register file
//! names `ebp`.

use std::process::Command;

const IR: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"
target triple = \"x86_64-unknown-linux-gnu\"

declare i32 @g(i32)
declare i32 @h(i32)

define i32 @f(i32 %a, i32 %b) {
  %x = call i32 @g(i32 %a)
  %y = call i32 @g(i32 %b)
  %z = call i32 @h(i32 %a)
  %s = add i32 %x, %y
  %t = add i32 %s, %z
  ret i32 %t
}
";

#[test]
fn three_values_across_calls_take_rbx_rbp_r14() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("f.ll");
    std::fs::write(&input, IR).unwrap();
    let done =
        Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args(["-O2", "-S", "-o", "-"]).arg(&input).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = String::from_utf8_lossy(&done.stdout);
    let pushed: Vec<&str> = asm.lines().filter_map(|line| line.trim().strip_prefix("push ")).collect();
    let mut pushed = pushed;
    pushed.sort_unstable();
    assert_eq!(pushed, ["r14", "rbp", "rbx"], "{asm}");
}
