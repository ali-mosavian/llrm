target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define void @f(ptr %p0, ptr %p1, ptr %p2, i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %a0 = getelementptr i64, ptr %p0, i32 %i
  %v0 = load i64, ptr %a0
  %w0 = trunc i64 %v0 to i32
  %a1 = getelementptr i16, ptr %p1, i32 %i
  %v1 = load i16, ptr %a1
  %w1 = sext i16 %v1 to i32
  %a2 = getelementptr i8, ptr %p2, i32 %i
  %s0 = add i32 0, %w0
  %s1 = add i32 %s0, %w1
  %t2 = trunc i32 %s1 to i8
  store i8 %t2, ptr %a2
  %next = add nuw i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
