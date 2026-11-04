target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

declare void @a()
declare void @b()

define void @both(i64 %n, i64 %m) {
  %x = icmp ne i64 %n, 0
  %y = icmp ne i64 %m, 0
  %c = select i1 %x, i1 %y, i1 false
  br i1 %c, label %t, label %f
t:
  call void @a()
  ret void
f:
  call void @b()
  ret void
}

define void @either(i64 %n, i64 %m) {
  %x = icmp ne i64 %n, 0
  %y = icmp ne i64 %m, 0
  %c = select i1 %x, i1 true, i1 %y
  br i1 %c, label %t, label %f
t:
  call void @a()
  ret void
f:
  call void @b()
  ret void
}
