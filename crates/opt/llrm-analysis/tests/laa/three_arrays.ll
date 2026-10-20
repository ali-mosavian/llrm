target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, ptr %b, ptr %c, ptr %d, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %pa = getelementptr inbounds i16, ptr %a, i64 %i
  %pb = getelementptr inbounds i16, ptr %b, i64 %i
  %pc = getelementptr inbounds i16, ptr %c, i64 %i
  %pd = getelementptr inbounds i16, ptr %d, i64 %i
  %x = load i16, ptr %pa, align 2
  %y = load i16, ptr %pb, align 2
  %z = add i16 %x, %y
  store i16 %z, ptr %pc, align 2
  store i16 %z, ptr %pd, align 2
  %i.next = add nuw nsw i64 %i, 1
  %e = icmp eq i64 %i.next, %n
  br i1 %e, label %exit, label %loop
exit:
  ret void
}
