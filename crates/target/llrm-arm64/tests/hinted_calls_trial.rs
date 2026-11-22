//! arm64 -Os: the pin's hint threshold inlines an `inlinehint` callee at any
//! size; the -Os size guard yields to it. QuickrBasic array.c `arry_esize`
//! (23 units, declared `inline`) was copied into four callers: 3,264 B against
//! gcc's 3,036. The -Os trial compiles the module with the calls kept and keeps
//! that object where it is smaller (INLINEHINT-GROWTH-TRIAL).

use std::process::Command;

const IR: &str = "target triple = \"aarch64-unknown-linux-gnu\"
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
@tab = external global [64 x i32]
define internal i32 @esize(ptr %p, i32 %k) inlinehint nounwind optsize {
  %a0 = load i32, ptr %p
  %a1 = getelementptr i32, ptr %p, i32 1
  %b0 = load i32, ptr %a1
  %c0 = and i32 %k, 7
  %t0 = getelementptr [64 x i32], ptr @tab, i32 0, i32 %c0
  %d0 = load i32, ptr %t0
  %m0 = mul i32 %a0, %d0
  %s0 = add i32 %m0, %b0
  %x0 = icmp ult i32 %s0, 100
  %y0 = select i1 %x0, i32 %s0, i32 100
  %c1 = lshr i32 %k, 3
  %t1 = getelementptr [64 x i32], ptr @tab, i32 0, i32 %c1
  %d1 = load i32, ptr %t1
  %m1 = mul i32 %y0, %d1
  %s1 = xor i32 %m1, %k
  %x1 = icmp sgt i32 %s1, 7
  %y1 = select i1 %x1, i32 %s1, i32 %m0
  %r = sub i32 %y1, %b0
  ret i32 %r
}
define i32 @one(ptr %p, i32 %k) nounwind optsize {
  %r = call i32 @esize(ptr %p, i32 %k)
  ret i32 %r
}
define i32 @two(ptr %p, i32 %k) nounwind optsize {
  %r = call i32 @esize(ptr %p, i32 %k)
  %s = add i32 %r, 1
  ret i32 %s
}
define i32 @three(ptr %p, i32 %k) nounwind optsize {
  %r = call i32 @esize(ptr %p, i32 %k)
  %s = shl i32 %r, 1
  ret i32 %s
}
define i32 @four(ptr %p, i32 %k) nounwind optsize {
  %r = call i32 @esize(ptr %p, i32 %k)
  %s = sub i32 0, %r
  ret i32 %s
}
";

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

#[test]
fn an_inlinehint_callee_copied_into_four_callers_stays_a_call_where_that_is_smaller() {
    let text = assembly(IR);
    assert!(text.contains("esize:"), "{text}");
}

/// A callee of arithmetic on its argument, called with constants: every copy
/// folds to a constant, so the copies are the smaller object and the trial
/// puts the calls back.
#[test]
fn an_inlinehint_callee_that_folds_at_each_site_stays_inlined() {
    let mut ir = String::from(
        "target triple = \"aarch64-unknown-linux-gnu\"
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
define internal i32 @mix(i32 %k) inlinehint nounwind optsize {
  %a = and i32 %k, 7
  %b = shl i32 %a, 2
  %c = add i32 %b, 5
  %d = mul i32 %c, 3
  %e = xor i32 %d, %k
  %f = lshr i32 %e, 1
  %g = sub i32 %f, %a
  %h = or i32 %g, %c
  %i = mul i32 %h, 7
  %j = add i32 %i, %e
  ret i32 %j
}
",
    );
    for (name, k) in [("one", 3), ("two", 5), ("three", 9), ("four", 12)] {
        ir.push_str(&format!(
            "define i32 @{name}() nounwind optsize {{\n  %r = call i32 @mix(i32 {k})\n  ret i32 %r\n}}\n"
        ));
    }
    let text = assembly(&ir);
    assert!(!text.contains("mix:"), "{text}");
}
