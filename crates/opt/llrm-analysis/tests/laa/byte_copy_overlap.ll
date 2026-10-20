target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, ptr %b, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %pr = getelementptr inbounds i8, ptr %b, i64 %i
  %v = load i8, ptr %pr, align 1
  %pw = getelementptr inbounds i8, ptr %a, i64 %i
  store i8 %v, ptr %pw, align 1
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
