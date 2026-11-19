target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

define i32 @f(ptr %p, ptr %q, i32 %n) {
entry:
  %a = load i16, ptr %p, align 2
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %at = getelementptr i16, ptr %q, i32 %i
  %b = load i16, ptr %at, align 2
  %lt = icmp slt i16 %a, %b
  %pick = select i1 %lt, i32 1, i32 2
  %sum = add i32 %acc, %pick
  %next = add i32 %i, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %exit, label %loop

exit:
  ret i32 %sum
}
