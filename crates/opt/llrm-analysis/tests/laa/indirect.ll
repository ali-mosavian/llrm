target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, ptr %b, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %pb = getelementptr inbounds i32, ptr %b, i64 %i
  %ix = load i32, ptr %pb, align 4
  %ixe = zext i32 %ix to i64
  %pa = getelementptr inbounds i32, ptr %a, i64 %ixe
  %v = load i32, ptr %pa, align 4
  %w = add i32 %v, 1
  store i32 %w, ptr %pa, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
