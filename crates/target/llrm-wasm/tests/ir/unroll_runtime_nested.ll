target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare void @report(double)
define void @sums(ptr %a, ptr %b) {
entry:
  br label %outer
outer:
  %n = phi i32 [ 0, %entry ], [ %n1, %latch ]
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %loop, label %latch
loop:
  %k = phi i32 [ 0, %outer ], [ %k1, %loop ]
  %s = phi double [ 0.0, %outer ], [ %s1, %loop ]
  %pa = getelementptr inbounds double, ptr %a, i32 %k
  %pb = getelementptr inbounds double, ptr %b, i32 %k
  %x = load double, ptr %pa, align 8
  %y = load double, ptr %pb, align 8
  %m = fmul double %x, %y
  %s1 = fadd double %s, %m
  %k1 = add nuw nsw i32 %k, 1
  %e = icmp eq i32 %k1, %n
  br i1 %e, label %latch, label %loop
latch:
  %r = phi double [ 0.0, %outer ], [ %s1, %loop ]
  call void @report(double %r)
  %n1 = add nuw nsw i32 %n, 1
  %d = icmp eq i32 %n1, 13
  br i1 %d, label %done, label %outer
done:
  ret void
}
