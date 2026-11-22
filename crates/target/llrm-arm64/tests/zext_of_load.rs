//! `AArch64MIPeepholeOpt::visitORR`: a load into a 32-bit register already
//! clears the upper half, so its `zext` to 64 bits is the same register.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const HEADER: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// matrix `sugg_add` -Os: the `i64` and `i32` zero-extensions of one `ldrh`
/// left `orr w22, wzr, w19` and a callee-saved register more than the pin (400 B
/// against 384 B).
#[test]
fn a_zero_extended_load_used_at_two_widths_has_no_copy() {
    let ir = "declare i32 @g(ptr)\ndeclare void @h(i32)\n\
define void @f(ptr %0, ptr %1) {\n\
  %3 = getelementptr inbounds nuw i8, ptr %0, i64 2\n  %4 = load i16, ptr %3, align 2\n\
  %5 = zext i16 %4 to i64\n  %6 = zext i16 %4 to i32\n\
  %7 = icmp eq i32 %6, 0\n  br i1 %7, label %out, label %loop\n\
loop:\n  %n = phi i64 [ %5, %2 ], [ %m, %loop ]\n  %c = call i32 @g(ptr %1)\n\
  %m = add nsw i64 %n, -1\n  %z = icmp eq i64 %m, 0\n  br i1 %z, label %out2, label %loop\n\
out2:\n  %k = call i32 @g(ptr %0)\n  %u = icmp ugt i32 %6, 191\n  br i1 %u, label %out, label %out3\n\
out3:\n  call void @h(i32 %6)\n  br label %out\n\
out:\n  ret void\n}\n";
    let text = driver::assembly(&format!("{HEADER}{ir}"), Platform::Linux, Level::Os).expect("lowers");
    assert!(!text.contains("orr w"), "{text}");
}
