target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

declare void @yes()
declare void @no()

define void @either(i32 %n, i32 %m) {
entry:
  %a = icmp eq i32 %n, 0
  %b = icmp eq i32 %m, 0
  %c = or i1 %a, %b
  br i1 %c, label %t, label %f
t:
  call void @yes()
  ret void
f:
  call void @no()
  ret void
}

define void @both(i32 %n, i32 %m, i32 %k) {
entry:
  %a = icmp slt i32 %n, 7
  %b = icmp sgt i32 %m, 9
  %c = icmp ne i32 %k, 3
  %ab = and i1 %a, %b
  %abc = and i1 %ab, %c
  br i1 %abc, label %t, label %f
t:
  call void @yes()
  ret void
f:
  call void @no()
  ret void
}

define i32 @into_phi(i32 %n, i32 %m) {
entry:
  %a = icmp ult i32 %n, 10
  %b = icmp ugt i32 %m, 20
  %c = or i1 %a, %b
  br i1 %c, label %join, label %other
other:
  call void @no()
  br label %join
join:
  %r = phi i32 [ 1, %entry ], [ 2, %other ]
  ret i32 %r
}
