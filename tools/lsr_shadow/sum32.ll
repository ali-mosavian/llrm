target datalayout = "e-m:o-i64:64-i128:128-n32:64-S128"

define i32 @k(ptr %a, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %p = getelementptr i32, ptr %a, i64 %i
  %v = load i32, ptr %p, align 4
  %sum = add i32 %acc, %v
  %next = add nuw i64 %i, 1
  %c = icmp ult i64 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %sum
}
