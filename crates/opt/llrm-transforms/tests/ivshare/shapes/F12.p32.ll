target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define void @f(ptr %p0, ptr %p1, i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i32 [ %n, %entry ], [ %next, %loop ]
  %a0 = getelementptr i32, ptr %p0, i32 %i
  %v0 = load i32, ptr %a0
  %a1 = getelementptr i32, ptr %p1, i32 %i
  %s0 = add i32 0, %v0
  store i32 %s0, ptr %a1
  %next = add nsw i32 %i, -1
  %c = icmp eq i32 %next, 0
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
