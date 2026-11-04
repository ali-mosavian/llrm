target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32"

define i32 @f(ptr %p0, ptr %p1, i16 %n) {
entry:
  %g = icmp sgt i16 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %acc2, %loop ]
  %a0 = getelementptr i32, ptr %p0, i16 %i
  %v0 = load i32, ptr %a0
  %a1 = getelementptr i32, ptr %p1, i16 %i
  %v1 = load i32, ptr %a1
  %s0 = add i32 0, %v0
  %s1 = add i32 %s0, %v1
  %acc2 = add i32 %acc, %s1
  %next = add nuw i16 %i, 1
  %c = icmp eq i16 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  %r = phi i32 [ 0, %entry ], [ %acc2, %loop ]
  ret i32 %r
}
