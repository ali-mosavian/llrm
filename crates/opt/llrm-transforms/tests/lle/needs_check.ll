target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, ptr %b, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %pa = getelementptr inbounds i32, ptr %a, i64 %i
  %x = load i32, ptr %pa, align 4
  %y = mul i32 %x, 3
  %j = add nuw nsw i64 %i, 1
  %pa1 = getelementptr inbounds i32, ptr %a, i64 %j
  store i32 %y, ptr %pa1, align 4
  %pb = getelementptr inbounds i32, ptr %b, i64 %i
  store i32 %x, ptr %pb, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
