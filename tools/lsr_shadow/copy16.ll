target datalayout = "e-m:o-i64:64-i128:128-n32:64-S128"

define void @k(ptr %d, ptr %s, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %sp = getelementptr i16, ptr %s, i64 %i
  %dp = getelementptr i16, ptr %d, i64 %i
  %v = load i16, ptr %sp, align 2
  store i16 %v, ptr %dp, align 2
  %next = add nuw i64 %i, 1
  %c = icmp ult i64 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
