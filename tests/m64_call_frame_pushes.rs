//! x86-64 passes stack arguments with pushes where the pin does
//! (`X86CallFrameOptimization`, `PUSH64`): two stack arguments are two pushes
//! and one `add rsp, 16` after the call (the prologue's own `push rax` aligns). Selection made `sub rsp, 16` and two
//! `mov [rsp+k], r` stores (os_locals_between_pushes -O2 was +21 B over the
//! pin's llc), because only m32 emitted the call-frame pseudos the pass reads.

mod common;

use std::process::Command;

const IR: &str = "target datalayout = \"@LAYOUT@\"
target triple = \"x86_64-unknown-linux-gnu\"

declare i64 @g(i64, i64, i64, i64, i64, i64, i64, i64)

define i64 @f(i64 %x) {
  %a = add i64 %x, 1
  %b = add i64 %x, 2
  %r = call i64 @g(i64 %x, i64 1, i64 2, i64 3, i64 4, i64 5, i64 %a, i64 %b)
  ret i64 %r
}
";

#[test]
fn two_stack_arguments_are_two_pushes_and_one_add() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("f.ll");
    std::fs::write(&input, IR.replace("@LAYOUT@", &common::datalayout("x86-m64"))).unwrap();
    let done =
        Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args(["-O2", "-S", "-o", "-"]).arg(&input).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = String::from_utf8_lossy(&done.stdout);
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    assert!(!lines.iter().any(|line| line.contains("[rsp")), "arguments stored, not pushed: {asm}");
    assert!(!lines.iter().any(|line| line.starts_with("sub") && line.contains("rsp")), "{asm}");
    assert!(lines.iter().any(|line| line.starts_with("add") && line.contains("rsp") && line.ends_with("16")), "{asm}");
}
