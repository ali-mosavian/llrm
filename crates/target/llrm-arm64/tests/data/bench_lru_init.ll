; lru's two init loops: constant stores to adjacent i16 elements, 3 and 7 trips.
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@bord = internal global [3 x i16] zeroinitializer
@bprev = internal global [3 x i16] zeroinitializer
@bnext = internal global [3 x i16] zeroinitializer
@bstamp = internal global [3 x i32] zeroinitializer
@lhead = internal global [7 x i16] zeroinitializer
@ltail = internal global [7 x i16] zeroinitializer

define void @init() {
entry:
  br label %one
one:
  %i = phi i64 [ 0, %entry ], [ %i.next, %one ]
  %p0 = getelementptr [3 x i16], ptr @bord, i64 0, i64 %i
  store i16 0, ptr %p0, align 2
  %p1 = getelementptr [3 x i16], ptr @bprev, i64 0, i64 %i
  store i16 -1, ptr %p1, align 2
  %p2 = getelementptr [3 x i16], ptr @bnext, i64 0, i64 %i
  store i16 -1, ptr %p2, align 2
  %p3 = getelementptr [3 x i32], ptr @bstamp, i64 0, i64 %i
  store i32 0, ptr %p3, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, 3
  br i1 %c, label %mid, label %one
mid:
  br label %two
two:
  %j = phi i64 [ 0, %mid ], [ %j.next, %two ]
  %q0 = getelementptr [7 x i16], ptr @lhead, i64 0, i64 %j
  store i16 -1, ptr %q0, align 2
  %q1 = getelementptr [7 x i16], ptr @ltail, i64 0, i64 %j
  store i16 -1, ptr %q1, align 2
  %j.next = add nuw nsw i64 %j, 1
  %d = icmp eq i64 %j.next, 7
  br i1 %d, label %done, label %two
done:
  ret void
}
