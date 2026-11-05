target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

define i32 @f(ptr %a) nounwind {
entry:
  br label %loop
loop:
  %i = phi i32 [ 1, %entry ], [ %next, %latch ]
  %p = getelementptr [4 x i8], ptr %a, i32 %i
  %pm = getelementptr i8, ptr %p, i32 -4
  %x0 = load i32, ptr %pm, align 4
  %x1 = load i32, ptr %p, align 4
  %c1 = icmp sgt i32 %x0, %x1
  br i1 %c1, label %bad, label %m1
m1:
  %p1 = getelementptr i8, ptr %p, i32 4
  %x2 = load i32, ptr %p1, align 4
  %c2 = icmp sgt i32 %x1, %x2
  br i1 %c2, label %bad, label %m2
m2:
  %p2 = getelementptr i8, ptr %p, i32 8
  %x3 = load i32, ptr %p2, align 4
  %c3 = icmp sgt i32 %x2, %x3
  br i1 %c3, label %bad, label %latch
latch:
  %next = add nuw nsw i32 %i, 3
  %d = icmp eq i32 %next, 1024
  br i1 %d, label %ok, label %loop
bad:
  ret i32 0
ok:
  ret i32 1
}
