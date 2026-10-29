target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

declare void @use(ptr)

define internal void @g(i64 %n) alwaysinline {
  %a = alloca i8, i64 %n, align 1
  call void @use(ptr %a)
  ret void
}

define void @h(i64 %m) {
entry:
  br label %l
l:
  %i = phi i64 [ 0, %entry ], [ %j, %l ]
  call void @g(i64 %m)
  %j = add i64 %i, 1
  %c = icmp ult i64 %j, 100
  br i1 %c, label %l, label %x
x:
  ret void
}
