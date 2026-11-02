//! `-fno-unroll-loops` reaches the mid-end of `llrm-arm64-llc`.

use std::process::Command;

const IR: &str = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @f(ptr %p) {\n\
entry:\n  br label %loop\n\
loop:\n  %i = phi i32 [0, %entry], [%n, %loop]\n  %s = phi i32 [0, %entry], [%t, %loop]\n\
  %a = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %a\n  %t = add i32 %s, %v\n\
  %n = add i32 %i, 1\n  %c = icmp ult i32 %n, 4\n  br i1 %c, label %loop, label %out\n\
out:\n  ret i32 %t\n}\n";

fn loads(flags: &[&str]) -> usize {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, IR).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2"])
        .args(flags)
        .arg("-o")
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap().lines().filter(|l| l.trim_start().starts_with("ldr")).count()
}

/// The flag was "unrecognized argument", so no mid-end comparison could be
/// made with unrolling off: a four-trip loop is four loads when unrolled, one
/// in the loop when not.
#[test]
fn a_constant_trip_loop_is_not_unrolled_with_the_flag() {
    let (default, off) = (loads(&[]), loads(&["-fno-unroll-loops", "-fno-peel-loops"]));
    assert!(off < default, "unrolled {default} loads, with the flag {off}");
}
