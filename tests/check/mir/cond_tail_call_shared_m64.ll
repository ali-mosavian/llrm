target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"
declare void @f()
declare void @h()
define void @g(i32 %n) {
  %a = icmp eq i32 %n, 3
  br i1 %a, label %cf, label %b
b:
  %c = icmp eq i32 %n, 2
  br i1 %c, label %ch, label %d
d:
  %e = icmp eq i32 %n, 1
  br i1 %e, label %cf, label %done
cf:
  tail call void @f()
  ret void
ch:
  tail call void @h()
  ret void
done:
  ret void
}
