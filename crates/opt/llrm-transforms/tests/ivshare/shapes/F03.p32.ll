target datalayout = "e-p:32:32-i64:32-f80:32-n8:16:32-S32"

define void @f(ptr %p0, ptr %p1, i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %a0 = getelementptr i8, ptr %p0, i32 %i
  %v0 = load i8, ptr %a0
  %w0 = sext i8 %v0 to i32
  %a1 = getelementptr i8, ptr %p1, i32 %i
  %s0 = add i32 0, %w0
  %t1 = trunc i32 %s0 to i8
  store i8 %t1, ptr %a1
  %next = add nuw i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
