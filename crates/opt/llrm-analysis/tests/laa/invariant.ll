target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, ptr %s, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %pa = getelementptr inbounds i32, ptr %a, i64 %i
  %v = load i32, ptr %pa, align 4
  %t = load i32, ptr %s, align 4
  %w = add i32 %v, %t
  store i32 %w, ptr %s, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
