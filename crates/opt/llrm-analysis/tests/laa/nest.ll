target datalayout = "e-m:e-p:64:64-i64:64-n8:16:32:64"

define void @f(ptr %a, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %outer, label %exit
outer:
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner
inner:
  %j = phi i64 [ 0, %outer ], [ %j.next, %inner ]
  %m = add nuw nsw i64 %i, %j
  %p = getelementptr inbounds i32, ptr %a, i64 %m
  %v = load i32, ptr %p, align 4
  %w = add i32 %v, 1
  store i32 %w, ptr %p, align 4
  %j.next = add nuw nsw i64 %j, 1
  %cj = icmp eq i64 %j.next, %n
  br i1 %cj, label %outer.latch, label %inner
outer.latch:
  %i.next = add nuw nsw i64 %i, 1
  %ci = icmp eq i64 %i.next, %n
  br i1 %ci, label %exit, label %outer
exit:
  ret void
}
