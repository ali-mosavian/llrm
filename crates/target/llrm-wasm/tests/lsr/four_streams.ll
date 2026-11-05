target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

define void @f(ptr %a, ptr %b, ptr %c, ptr %d, i32 %n) nounwind {
entry:
  br label %outer
outer:
  %j = phi i32 [ 0, %entry ], [ %j.next, %inner.exit ]
  br label %inner
inner:
  %i = phi i32 [ 0, %outer ], [ %i.next, %inner ]
  %pa = getelementptr inbounds double, ptr %a, i32 %i
  %pb = getelementptr inbounds double, ptr %b, i32 %i
  %pc = getelementptr inbounds double, ptr %c, i32 %i
  %pd = getelementptr inbounds double, ptr %d, i32 %i
  %x = load double, ptr %pa, align 8
  %y = load double, ptr %pb, align 8
  %z = fadd double %x, %y
  store double %z, ptr %pc, align 8
  %w = load double, ptr %pd, align 8
  %v = fmul double %w, %z
  store double %v, ptr %pd, align 8
  %i.next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i.next, 4
  br i1 %done, label %inner.exit, label %inner
inner.exit:
  %j.next = add nuw nsw i32 %j, 1
  %more = icmp eq i32 %j.next, %n
  br i1 %more, label %exit, label %outer
exit:
  ret void
}
