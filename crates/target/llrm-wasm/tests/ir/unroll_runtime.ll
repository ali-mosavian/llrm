target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define double @dot(ptr %a, ptr %b, i32 %n) {
entry:
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %loop, label %done
loop:
  %k = phi i32 [ 0, %entry ], [ %k1, %loop ]
  %s = phi double [ 0.0, %entry ], [ %s1, %loop ]
  %pa = getelementptr inbounds double, ptr %a, i32 %k
  %pb = getelementptr inbounds double, ptr %b, i32 %k
  %x = load double, ptr %pa, align 8
  %y = load double, ptr %pb, align 8
  %m = fmul double %x, %y
  %s1 = fadd double %s, %m
  %k1 = add nuw nsw i32 %k, 1
  %e = icmp eq i32 %k1, %n
  br i1 %e, label %done, label %loop
done:
  %r = phi double [ 0.0, %entry ], [ %s1, %loop ]
  ret double %r
}
