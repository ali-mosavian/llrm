target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

declare i32 @g(i32)

define i32 @f(i32 %x) {
entry:
  %z = icmp eq i32 %x, 0
  br i1 %z, label %done, label %call
call:
  %r = call i32 @g(i32 %x)
  %s = add i32 %r, 1
  br label %done
done:
  %v = phi i32 [ 1, %entry ], [ %s, %call ]
  ret i32 %v
}
