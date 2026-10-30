target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %i2 = shl nuw nsw i64 %i, 1
  %pr = getelementptr inbounds i32, ptr %a, i64 %i2
  %v = load i32, ptr %pr, align 4
  %i3 = or disjoint i64 %i2, 1
  %pw = getelementptr inbounds i32, ptr %a, i64 %i3
  store i32 %v, ptr %pw, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
