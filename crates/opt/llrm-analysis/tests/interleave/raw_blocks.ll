target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define i32 @f(ptr %a, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %i2 = shl nuw nsw i64 %i, 1
  %p0 = getelementptr inbounds i32, ptr %a, i64 %i2
  %v0 = load i32, ptr %p0, align 4
  %i3 = or disjoint i64 %i2, 1
  %p1 = getelementptr inbounds i32, ptr %a, i64 %i3
  store i32 %v0, ptr %p1, align 4
  %v1 = load i32, ptr %p1, align 4
  %t = add i32 %s, %v0
  %s2 = add i32 %t, %v1
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret i32 %s2
}
