target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@a = global [64 x i16] zeroinitializer, align 2
@counts = internal constant [6 x i16] [i16 0, i16 1, i16 2, i16 17, i16 63, i16 64], align 2
declare void @report(i64)
define void @fillw(i16 %n, i16 %v) {
entry:
  %c = icmp sgt i16 %n, 0
  br i1 %c, label %loop, label %exit
loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %loop ]
  %z = zext nneg i16 %i to i64
  %p = getelementptr inbounds i16, ptr @a, i64 %z
  store i16 %v, ptr %p, align 2
  %i2 = add nuw nsw i16 %i, 1
  %d = icmp slt i16 %i2, %n
  br i1 %d, label %loop, label %exit
exit:
  ret void
}
define i32 @main() {
entry:
  br label %outer
outer:
  %t = phi i64 [ 0, %entry ], [ %t2, %outer ]
  %q = getelementptr inbounds i16, ptr @counts, i64 %t
  %n = load i16, ptr %q, align 2
  call void @fillw(i16 %n, i16 4660)
  %x = sext i16 %n to i64
  call void @report(i64 %x)
  %t2 = add nuw nsw i64 %t, 1
  %e = icmp eq i64 %t2, 6
  br i1 %e, label %done, label %outer
done:
  ret i32 0
}
