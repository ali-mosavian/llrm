//! A constant a loop returns from inside it is made once, before the loop
//! (the pin's post-allocation MachineLICM). Ours made `xor eax, eax` on every
//! trip of `for (...) if (x == c) return 0; return 1;`: queens -O1 executed
//! 166,987 instructions against the pin's 160,755, the whole gap one `xor` per
//! trip (5,720 of 6,232).

mod common;

use std::process::Command;

const IR: &str = "target datalayout = \"@LAYOUT@\"
target triple = \"x86_64-unknown-linux-gnu\"

define i32 @safe(ptr %q, i32 %row, i32 %col) {
entry:
  %any = icmp sgt i32 %row, 0
  br i1 %any, label %loop, label %ret
loop:
  %i = phi i32 [ 0, %entry ], [ %n, %next ]
  %p = getelementptr i32, ptr %q, i32 %i
  %v = load i32, ptr %p
  %hit = icmp eq i32 %v, %col
  br i1 %hit, label %ret, label %next
next:
  %n = add i32 %i, 1
  %more = icmp slt i32 %n, %row
  br i1 %more, label %loop, label %ret
ret:
  %r = phi i32 [ 1, %entry ], [ @CONST@, %loop ], [ 1, %next ]
  ret i32 %r
}
";

/// The lines of the loop: from a label to the jump back to it.
fn loop_lines(asm: &str) -> Vec<&str> {
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    for (at, line) in lines.iter().enumerate() {
        let Some(target) = line.split_whitespace().nth(1).filter(|_| line.starts_with('j')) else { continue };
        if let Some(label) = lines[..at].iter().position(|earlier| earlier.strip_suffix(':') == Some(target)) {
            return lines[label..=at].to_vec();
        }
    }
    panic!("premise: the function has a loop\n{asm}")
}

fn assemble(constant: i32) -> String {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("safe.ll");
    let text = IR.replace("@LAYOUT@", &common::datalayout("x86-m64")).replace("@CONST@", &constant.to_string());
    std::fs::write(&input, text).unwrap();
    let done =
        Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args(["-O1", "-S", "-o", "-"]).arg(&input).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stdout).into_owned()
}

#[test]
fn a_nonzero_constant_returned_from_inside_a_loop_is_made_before_it() {
    let asm = assemble(7);
    assert!(asm.contains("eax, 7"), "premise: the constant is made\n{asm}");
    let made = loop_lines(&asm).into_iter().filter(|line| line.contains("eax, 7")).count();
    assert_eq!(made, 0, "made on every trip of the loop:\n{asm}");
}

#[test]
fn a_zero_returned_from_inside_a_loop_is_made_before_it() {
    let asm = assemble(0);
    assert!(asm.contains("xor eax, eax") || asm.contains("eax, 0"), "premise: the zero is made\n{asm}");
    let made =
        loop_lines(&asm).into_iter().filter(|line| line.contains("xor eax, eax") || line.contains("eax, 0")).count();
    assert_eq!(made, 0, "made on every trip of the loop:\n{asm}");
}
