target datalayout = "e-m:o-i64:64-i128:128-n32:64-S128"

define void @k(ptr %x, ptr %y, ptr %z, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %xp = getelementptr i32, ptr %x, i64 %i
  %yp = getelementptr i32, ptr %y, i64 %i
  %zp = getelementptr i32, ptr %z, i64 %i
  %a = load i32, ptr %xp, align 4
  %b = load i32, ptr %yp, align 4
  %s = add i32 %a, %b
  store i32 %s, ptr %zp, align 4
  %next = add nuw i64 %i, 1
  %c = icmp ult i64 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
