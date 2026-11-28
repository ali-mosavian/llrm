//! arm64 -O1 held a speed caller to the -Os rule that an inline must not
//! grow the code: QB's `static inline` `hot_int` and `hot_operand` stayed calls
//! in the interpreter loop, and int_math ran 586M instructions against clang
//! -O1's 270M. -O1 keeps the pin's threshold, which inlines them.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const IR: &str = r#"target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define internal i32 @op(i32 %a, i32 %b, i32 %k) {
entry:
  switch i32 %k, label %other [
    i32 0, label %add
    i32 1, label %sub
    i32 2, label %mul
    i32 3, label %xor
  ]
add:
  %t0 = add i32 %a, %b
  %u0 = mul i32 %t0, %k
  %v0 = xor i32 %u0, 85
  %s0 = add i32 %v0, %a
  br label %done
sub:
  %t1 = sub i32 %a, %b
  %u1 = mul i32 %t1, %k
  %v1 = or i32 %u1, 7
  %s1 = sub i32 %v1, %b
  br label %done
mul:
  %s2 = mul i32 %a, %b
  br label %done
xor:
  %s3 = xor i32 %a, %b
  br label %done
other:
  %s4 = shl i32 %a, %b
  br label %done
done:
  %r = phi i32 [ %s0, %add ], [ %s1, %sub ], [ %s2, %mul ], [ %s3, %xor ], [ %s4, %other ]
  ret i32 %r
}

define i32 @run(ptr %code, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %acc = phi i32 [ 1, %entry ], [ %acc2, %loop ]
  %at = getelementptr i32, ptr %code, i32 %i
  %k = load i32, ptr %at
  %acc1 = call i32 @op(i32 %acc, i32 %i, i32 %k)
  %accb = call i32 @op(i32 %acc1, i32 5, i32 %k)
  %accc = call i32 @op(i32 %accb, i32 %n, i32 %k)
  %acc2 = call i32 @op(i32 %accc, i32 3, i32 %k)
  %i.next = add i32 %i, 1
  %more = icmp slt i32 %i.next, %n
  br i1 %more, label %loop, label %exit
exit:
  ret i32 %acc2
}
"#;

#[test]
fn an_o1_loop_inlines_a_helper_that_grows_the_code() {
    let done = driver::object_with(IR, Platform::Linux, "t", Level::O1, false).expect("lowers");
    let defined: Vec<&str> = done.object.symbols.iter().map(|one| one.name.as_str()).collect();
    assert!(!defined.contains(&"op"), "op stayed a call at -O1: {defined:?}");
}
