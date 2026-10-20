target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %j = add nuw nsw i64 %i, 200
  %pr = getelementptr inbounds i32, ptr %a, i64 %j
  %v = load i32, ptr %pr, align 4
  %pw = getelementptr inbounds i32, ptr %a, i64 %i
  store i32 %v, ptr %pw, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, 100
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
