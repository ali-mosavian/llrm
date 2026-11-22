//! arm64 -Os: the size guard gives way where the pin's inline cost is below
//! zero, so a callee of a few instructions (QuickrBasic code.h `code_byte`:
//! a bounds check and a store) was copied into every caller although it
//! grows the code: codegen.c 7,008 B against gcc's 6,832, code.c 1,680
//! against 1,348. The -Os trial compiles the module with those calls kept and
//! keeps that object where it is smaller (INLINEHINT-GROWTH-TRIAL).

use std::process::Command;

fn assembly(ir: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, ir).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

fn module(callers: usize) -> String {
    let mut ir = String::from(
        "target triple = \"aarch64-unknown-linux-gnu\"
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
%code = type { ptr, ptr, ptr, i8 }
define internal void @put(ptr %c, i8 %b) nounwind optsize {
  %outp = getelementptr %code, ptr %c, i32 0, i32 1
  %out = load ptr, ptr %outp
  %limp = getelementptr %code, ptr %c, i32 0, i32 2
  %lim = load ptr, ptr %limp
  %full = icmp uge ptr %out, %lim
  br i1 %full, label %set, label %store
set:
  %fp = getelementptr %code, ptr %c, i32 0, i32 3
  store i8 1, ptr %fp
  ret void
store:
  %next = getelementptr i8, ptr %out, i32 1
  store ptr %next, ptr %outp
  store i8 %b, ptr %out
  ret void
}
",
    );
    for n in 0..callers {
        ir.push_str(&format!(
            "define void @op{n}(ptr %c, i8 %x) nounwind optsize {{\n  call void @put(ptr %c, i8 {n})\n  call void @put(ptr %c, i8 %x)\n  ret void\n}}\n"
        ));
    }
    ir
}

#[test]
fn a_shrinking_callee_copied_into_many_callers_stays_a_call_where_that_is_smaller() {
    let text = assembly(&module(6));
    assert!(text.contains("put:"), "{text}");
}

/// A callee called from one place goes with its call: inlining it is the
/// smaller object and the trial keeps the copy.
#[test]
fn a_callee_with_one_caller_is_inlined() {
    let text = assembly(&module(1).replacen("call void @put(ptr %c, i8 %x)\n", "", 1));
    assert!(!text.contains("put:"), "{text}");
}
