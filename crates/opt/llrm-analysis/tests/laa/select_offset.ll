target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, ptr %d, i1 %cond, i64 %k, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %off = select i1 %cond, i64 %i, i64 %k
  %ps = getelementptr inbounds i32, ptr %a, i64 %off
  %v = load i32, ptr %ps, align 4
  %pd = getelementptr inbounds i32, ptr %d, i64 %i
  store i32 %v, ptr %pd, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
