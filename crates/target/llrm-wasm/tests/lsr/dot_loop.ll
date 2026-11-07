target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

define i32 @dot(ptr %a, ptr %b) nounwind {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s.next, %loop ]
  %pa = getelementptr inbounds i32, ptr %a, i32 %i
  %pb = getelementptr inbounds i32, ptr %b, i32 %i
  %x = load i32, ptr %pa, align 4
  %y = load i32, ptr %pb, align 4
  %m = mul i32 %x, %y
  %s.next = add i32 %s, %m
  %i.next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i.next, 100
  br i1 %done, label %exit, label %loop
exit:
  ret i32 %s.next
}
