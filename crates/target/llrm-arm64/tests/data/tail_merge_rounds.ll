target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare void @own(ptr, ptr)
@mem = external global ptr
define void @f(ptr %c, ptr %n, i32 %k) {
  switch i32 %k, label %out [ i32 1, label %a  i32 2, label %b ]
a:
  call void @own(ptr %c, ptr %n)
  %la = load ptr, ptr %c, align 8
  %va = load i8, ptr %la, align 1
  %ca = icmp sgt i8 %va, -1
  br i1 %ca, label %a2, label %join
a2:
  %xa = load ptr, ptr @mem, align 8
  %wa = load i32, ptr %xa, align 4
  %ta = icmp eq i32 %wa, 7
  br label %join
b:
  call void @own(ptr %c, ptr %n)
  %lb = load ptr, ptr %c, align 8
  %vb = load i8, ptr %lb, align 1
  %cb = icmp sgt i8 %vb, -1
  br i1 %cb, label %b2, label %join
b2:
  %xb = load ptr, ptr @mem, align 8
  %wb = load i32, ptr %xb, align 4
  %tb = icmp eq i32 %wb, 7
  br label %join
join:
  %p = phi i1 [ false, %a ], [ %ta, %a2 ], [ false, %b ], [ %tb, %b2 ]
  %q = getelementptr i8, ptr %n, i64 58
  %z = zext i1 %p to i8
  store i8 %z, ptr %q, align 1
  br label %out
out:
  ret void
}
