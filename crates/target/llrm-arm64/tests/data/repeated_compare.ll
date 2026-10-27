target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

define void @both(double %a, double %b, ptr %p) {
  %lt = fcmp olt double %a, %b
  %gt = fcmp ogt double %a, %b
  %x = select i1 %lt, i32 10, i32 20
  %y = select i1 %gt, i32 30, i32 40
  store volatile i32 %x, ptr %p, align 4
  store volatile i32 %y, ptr %p, align 4
  ret void
}

define void @redefined(i32 %a, i32 %b, ptr %p) {
  %lt = icmp slt i32 %a, %b
  %x = select i1 %lt, i32 10, i32 20
  store volatile i32 %x, ptr %p, align 4
  %c = add i32 %a, 1
  %lt2 = icmp slt i32 %c, %b
  %y = select i1 %lt2, i32 10, i32 20
  store volatile i32 %y, ptr %p, align 4
  ret void
}
