target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@p = global i32 0, align 4
@q = global i32 0, align 4
declare void @g(i32)
declare void @h(i32, i32)
declare void @k(i32)
define void @f(i32 %a, i32 %b, i32 %c) {
entry:
  switch i32 %c, label %z [ i32 0, label %x
                           i32 1, label %y ]
x:
  store volatile i32 %a, ptr @p, align 4
  call void @g(i32 11)
  call void @h(i32 22, i32 33)
  call void @k(i32 44)
  br label %m
y:
  store volatile i32 %b, ptr @q, align 4
  call void @g(i32 11)
  call void @h(i32 22, i32 33)
  call void @k(i32 44)
  br label %m
z:
  store volatile i32 %c, ptr @p, align 4
  br label %m
m:
  ret void
}
