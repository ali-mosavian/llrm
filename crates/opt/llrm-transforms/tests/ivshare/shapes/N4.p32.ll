target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define void @f(ptr %p0, ptr %p1) {
entry:
  br label %loop
loop:
  %i = phi i8 [ 0, %entry ], [ %next, %loop ]
  %e0 = zext i8 %i to i32
  %a0 = getelementptr i8, ptr %p0, i32 %e0
  %v0 = load i8, ptr %a0
  %w0 = sext i8 %v0 to i32
  %e1 = zext i8 %i to i32
  %a1 = getelementptr i8, ptr %p1, i32 %e1
  %s0 = add i32 0, %w0
  %t1 = trunc i32 %s0 to i8
  store i8 %t1, ptr %a1
  %next = add nuw i8 %i, 1
  %c = icmp eq i8 %next, -6
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
