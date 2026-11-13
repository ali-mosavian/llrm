target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, i32 %x, i32 %y, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %i2 = shl nuw nsw i64 %i, 1
  %p0 = getelementptr inbounds i32, ptr %a, i64 %i2
  store i32 %x, ptr %p0, align 4
  %i3 = or disjoint i64 %i2, 1
  %p1 = getelementptr inbounds i32, ptr %a, i64 %i3
  store i32 %y, ptr %p1, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
